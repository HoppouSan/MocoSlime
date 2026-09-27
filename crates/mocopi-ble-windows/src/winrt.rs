//! Real WinRT BLE implementation using the `windows` crate.
//!
//! WinRT `IAsyncOperation::get()` is blocking, so every public async entry
//! point offloads the blocking work with `spawn_blocking` and applies a
//! `tokio::time::timeout`. Notification callbacks are synchronous; they
//! parse packets synchronously and forward them through a Tokio unbounded
//! channel so no `.await` happens inside the callback.

use crate::device::DeviceInfo;
use crate::error::{BleError, Result};
use mocopi_protocol::{constants::*, decode_packet, DecodedPacket};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;
use windows::{
    core::{GUID, HSTRING},
    Devices::Bluetooth::GenericAttributeProfile::{
        GattCharacteristic, GattCharacteristicProperties,
        GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus,
        GattDeviceService, GattValueChangedEventArgs, GattWriteOption,
    },
    Devices::{
        Bluetooth::Advertisement::{
            BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
            BluetoothLEScanningMode,
        },
        Bluetooth::{
            BluetoothAdapter, BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
        },
        Enumeration::{DeviceInformation, DeviceInformationUpdate, DeviceWatcher},
    },
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Storage::Streams::{DataReader, DataWriter, IBuffer},
};

pub(crate) fn ble_guid(uuid_str: &str) -> Result<GUID> {
    let parsed = uuid::Uuid::parse_str(uuid_str).map_err(|e| BleError::InvalidAddress {
        address: uuid_str.to_string(),
        reason: format!("invalid UUID: {e}"),
    })?;
    Ok(GUID::from_u128(parsed.as_u128()))
}

pub(crate) fn address_to_u64(address: &str) -> Result<u64> {
    let clean: String = address.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 12 {
        return Err(BleError::InvalidAddress {
            address: address.to_string(),
            reason: "expected 6 hex bytes (12 hex digits)".to_string(),
        });
    }
    u64::from_str_radix(&clean, 16).map_err(|e| BleError::InvalidAddress {
        address: address.to_string(),
        reason: e.to_string(),
    })
}

pub(crate) fn u64_to_address(addr: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (addr >> 40) & 0xFF,
        (addr >> 32) & 0xFF,
        (addr >> 24) & 0xFF,
        (addr >> 16) & 0xFF,
        (addr >> 8) & 0xFF,
        addr & 0xFF
    )
}

fn buffer_to_vec(buffer: &IBuffer) -> Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let len = reader.UnconsumedBufferLength()? as usize;
    let mut out = vec![0u8; len];
    reader.ReadBytes(&mut out)?;
    let _ = reader.Close();
    Ok(out)
}

fn vec_to_buffer(data: &[u8]) -> Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(data)?;
    Ok(writer.DetachBuffer()?)
}

fn vector_view_to_vec<T>(view: &windows::Foundation::Collections::IVectorView<T>) -> Result<Vec<T>>
where
    T: windows::core::RuntimeType,
{
    let size = view.Size()?;
    let mut out = Vec::with_capacity(size as usize);
    for i in 0..size {
        out.push(view.GetAt(i)?);
    }
    Ok(out)
}

fn ensure_adapter_present() -> Result<()> {
    BluetoothAdapter::GetDefaultAsync()
        .and_then(|op| op.get())
        .map(|_| ())
        .map_err(|_| BleError::AdapterNotFound)
}

/// Public adapter probe for diagnostics: `Ok(())` when a Bluetooth
/// adapter exists, otherwise the concrete [`BleError`]
/// (`AdapterNotFound`, …) so support bundles show the real cause.
pub fn adapter_status() -> Result<()> {
    ensure_adapter_present()
}

fn is_mocopi_name(name: &str) -> bool {
    name.contains("QM-SS1") || name.contains("Mocopi")
}

/// Cached enumeration: every BLE device Windows knows about (paired or
/// previously seen). Deliberately NOT name-filtered here: a tracker that
/// is paired/connected in Windows Settings can show up with an empty or
/// system-assigned name, and filtering too early is exactly how paired
/// trackers get missed. Name/service matching happens in
/// [`blocking_scan_full`] after a refresh.
fn blocking_enumerate() -> Result<Vec<(String, String, bool)>> {
    let selector = BluetoothLEDevice::GetDeviceSelector()?;
    let collection = DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.get())?;
    let size = collection.Size().unwrap_or(0);
    tracing::info!("WinRT: enumeration found {} total BLE device(s)", size);
    let mut out = Vec::new();
    for i in 0..size {
        let info = match collection.GetAt(i) {
            Ok(info) => info,
            Err(_) => continue,
        };
        let name = info.Name().map(|n| n.to_string()).unwrap_or_default();
        let id = match info.Id() {
            Ok(id) => id.to_string(),
            Err(_) => continue,
        };
        let paired = info.Pairing().and_then(|p| p.IsPaired()).unwrap_or(false);
        tracing::debug!("WinRT: enum device name='{name}' paired={paired}");
        out.push((id, name, paired));
    }
    Ok(out)
}

/// Live watcher scan: `DeviceWatcher` reports devices as their
/// advertisements arrive during `window`. This catches trackers that
/// are powered on now but missing from the Windows device cache
/// (the main "scan finds nothing" cause).
fn blocking_watcher_collect(window: Duration) -> (Vec<(String, String)>, HashMap<u64, i16>) {
    let selector = match BluetoothLEDevice::GetDeviceSelector() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("WinRT: watcher selector failed: {e:?}");
            return (Vec::new(), HashMap::new());
        }
    };
    let watcher = match DeviceInformation::CreateWatcherAqsFilter(&selector) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("WinRT: watcher creation failed: {e:?}");
            return (Vec::new(), HashMap::new());
        }
    };
    let seen: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let rssi_by_address: Arc<Mutex<HashMap<u64, i16>>> = Arc::new(Mutex::new(HashMap::new()));

    // DeviceWatcher supplies stable device IDs/names but not RSSI. The LE
    // advertisement watcher is the Windows API that reports RSSI in dBm.
    let rssi_watcher = BluetoothLEAdvertisementWatcher::new().ok();
    let rssi_token = rssi_watcher.as_ref().and_then(|watcher| {
        let values = rssi_by_address.clone();
        watcher
            .Received(&TypedEventHandler::new(
                move |_: &Option<BluetoothLEAdvertisementWatcher>,
                      args: &Option<BluetoothLEAdvertisementReceivedEventArgs>| {
                    if let Some(args) = args.as_ref() {
                        if let (Ok(address), Ok(rssi)) =
                            (args.BluetoothAddress(), args.RawSignalStrengthInDBm())
                        {
                            if address != 0 && (-127..=0).contains(&rssi) {
                                if let Ok(mut values) = values.lock() {
                                    // Keep the strongest sample from this scan
                                    // to avoid showing a momentary radio dip.
                                    values
                                        .entry(address)
                                        .and_modify(|old| *old = (*old).max(rssi))
                                        .or_insert(rssi);
                                }
                            }
                        }
                    }
                    Ok(())
                },
            ))
            .ok()
    });
    if let Some(watcher) = rssi_watcher.as_ref() {
        if watcher
            .SetScanningMode(BluetoothLEScanningMode::Active)
            .is_ok()
            && watcher.Start().is_ok()
        {
            tracing::info!("WinRT: RSSI advertisement watcher started");
        } else {
            tracing::warn!("WinRT: RSSI advertisement watcher could not start");
        }
    }

    let seen_added = seen.clone();
    let added_token: Option<EventRegistrationToken> = watcher
        .Added(&TypedEventHandler::new(
            move |_sender: &Option<DeviceWatcher>, info: &Option<DeviceInformation>| {
                let (Some(_), Some(info)) = (_sender.as_ref(), info.as_ref()) else {
                    return Ok(());
                };
                // Store every device; names often arrive late/empty and
                // filtering happens after the refresh step.
                let name = info.Name().map(|n| n.to_string()).unwrap_or_default();
                let id = info.Id().map(|v| v.to_string()).unwrap_or_default();
                if id.is_empty() {
                    return Ok(());
                }
                if let Ok(mut guard) = seen_added.lock() {
                    guard
                        .entry(id)
                        .and_modify(|n| {
                            if n.is_empty() {
                                *n = name.clone();
                            }
                        })
                        .or_insert(name);
                }
                Ok(())
            },
        ))
        .ok();
    let seen_updated = seen.clone();
    let updated_token: Option<EventRegistrationToken> = watcher
        .Updated(&TypedEventHandler::new(
            move |_sender: &Option<DeviceWatcher>, info: &Option<DeviceInformationUpdate>| {
                let Some(info) = info.as_ref() else {
                    return Ok(());
                };
                // Updated args carry the Id; the name may arrive late via
                // update, so store the Id and resolve the name afterwards.
                let id = info.Id().map(|v| v.to_string()).unwrap_or_default();
                if id.is_empty() {
                    return Ok(());
                }
                if let Ok(mut guard) = seen_updated.lock() {
                    guard.entry(id).or_insert_with(String::new);
                }
                Ok(())
            },
        ))
        .ok();

    if let Err(e) = watcher.Start() {
        tracing::warn!("WinRT: watcher start failed: {e:?}");
    } else {
        std::thread::sleep(window);
        let _ = watcher.Stop();
    }
    if let Some(t) = added_token {
        let _ = watcher.RemoveAdded(t);
    }
    if let Some(t) = updated_token {
        let _ = watcher.RemoveUpdated(t);
    }
    if let Some(watcher) = rssi_watcher {
        let _ = watcher.Stop();
        if let Some(token) = rssi_token {
            let _ = watcher.RemoveReceived(token);
        }
    }
    let collected: Vec<(String, String)> = match seen.lock() {
        Ok(guard) => guard.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Err(_) => Vec::new(),
    };
    let rssi_by_address = rssi_by_address
        .lock()
        .map(|values| values.clone())
        .unwrap_or_default();
    tracing::info!(
        "WinRT: RSSI watcher captured {} unique advertiser(s)",
        rssi_by_address.len()
    );
    (collected, rssi_by_address)
}

fn resolve_address(device_id: &str) -> Option<(u64, String)> {
    let id_h: HSTRING = device_id.into();
    let device = BluetoothLEDevice::FromIdAsync(&id_h)
        .and_then(|op| op.get())
        .ok()?;
    let addr = device.BluetoothAddress().ok()?;
    let name = device.Name().map(|n| n.to_string()).unwrap_or_default();
    Some((addr, name))
}

/// Re-read name + pairing state for a device whose cached name is empty
/// or stale (common for trackers paired in Windows Settings: the
/// enumeration snapshot can lag behind the live device).
fn refresh_device_info(device_id: &str) -> Option<(String, bool)> {
    let id_h: HSTRING = device_id.into();
    let info = DeviceInformation::CreateFromIdAsync(&id_h)
        .and_then(|op| op.get())
        .ok()?;
    let name = info.Name().map(|n| n.to_string()).unwrap_or_default();
    let paired = info.Pairing().and_then(|p| p.IsPaired()).unwrap_or(false);
    Some((name, paired))
}

/// GATT fallback: a tracker paired/connected in Windows Settings can
/// report an empty or system-assigned name. The Mocopi command service
/// UUID identifies it reliably. Uses the fast cached lookup and runs on
/// a worker thread with a bounded wait in the caller, so an unreachable
/// device can't stall the scan.
fn has_mocopi_service(device_id: &str) -> bool {
    let id_h: HSTRING = device_id.into();
    let device = match BluetoothLEDevice::FromIdAsync(&id_h).and_then(|op| op.get()) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let services = match blocking_get_services_cached(&device) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let want = match ble_guid(CMD_SERVICE_UUID) {
        Ok(g) => g,
        Err(_) => return false,
    };
    services
        .iter()
        .any(|s| s.Uuid().map(|u| u == want).unwrap_or(false))
}

fn blocking_scan_full(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    ensure_adapter_present()?;
    // Reserve ~2s for enumeration + resolution + GATT fallback probes;
    // the rest is live watcher time (min 2s so short timeouts still
    // catch adverts).
    let watch_window = timeout
        .checked_sub(Duration::from_millis(2000))
        .unwrap_or(Duration::from_secs(2))
        .max(Duration::from_secs(2));
    tracing::debug!(
        "WinRT: live watcher window {}ms (total budget {}ms)",
        watch_window.as_millis(),
        timeout.as_millis()
    );
    let (watched, rssi_by_address) = blocking_watcher_collect(watch_window);
    tracing::debug!("WinRT: watcher saw {} candidate(s)", watched.len());
    let enumerated = blocking_enumerate().unwrap_or_default();

    // Merge by device Id, preferring non-empty names and paired=true.
    let mut by_id: HashMap<String, (String, bool)> = HashMap::new();
    for (id, wname) in watched {
        by_id
            .entry(id)
            .and_modify(|(n, _)| {
                if n.is_empty() {
                    *n = wname.clone();
                }
            })
            .or_insert((wname, false));
    }
    for (id, ename, paired) in enumerated {
        by_id
            .entry(id)
            .and_modify(|(n, p)| {
                if n.is_empty() {
                    *n = ename.clone();
                }
                *p = *p || paired;
            })
            .or_insert((ename, paired));
    }
    tracing::info!("WinRT: {} unique BLE device(s) to evaluate", by_id.len());

    // Refresh stale/empty names straight from the OS before matching.
    for (id, (name, paired)) in by_id.iter_mut() {
        if name.is_empty() || !is_mocopi_name(name) {
            if let Some((fresh_name, fresh_paired)) = refresh_device_info(id) {
                if !fresh_name.is_empty() {
                    tracing::debug!("WinRT: refreshed name for {id}: '{name}' -> '{fresh_name}'");
                    *name = fresh_name;
                }
                *paired = *paired || fresh_paired;
            }
        }
    }

    // Resolve addresses; split into name matches vs. paired unknowns.
    struct Candidate {
        name: String,
        paired: bool,
        addr: u64,
    }
    let mut matched: Vec<Candidate> = Vec::new();
    let mut probe_ids: Vec<(String, u64)> = Vec::new();
    let mut seen_addr: HashMap<u64, bool> = HashMap::new();
    for (id, (name, paired)) in by_id {
        let (addr_u64, live_name) = match resolve_address(&id) {
            Some(v) => v,
            None => {
                tracing::debug!("WinRT: address resolve failed for {id}");
                continue;
            }
        };
        if seen_addr.insert(addr_u64, true).is_some() {
            continue;
        }
        let name = if name.is_empty() { live_name } else { name };
        if is_mocopi_name(&name) {
            matched.push(Candidate {
                name,
                paired,
                addr: addr_u64,
            });
        } else if paired {
            // Paired/connected in Windows but unrecognized name: candidate
            // for the GATT service probe below.
            tracing::debug!(
                "WinRT: paired device with foreign name '{name}' queued for GATT probe"
            );
            probe_ids.push((id, addr_u64));
        }
    }

    // GATT probe for paired unknowns, in parallel with a hard overall
    // budget so one stuck device can't stall the scan.
    if !probe_ids.is_empty() {
        tracing::info!(
            "WinRT: probing {} paired device(s) for Mocopi GATT service",
            probe_ids.len()
        );
        let (tx, rx) = std::sync::mpsc::channel::<(String, u64, bool)>();
        for (id, addr) in probe_ids.into_iter().take(8) {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let hit = has_mocopi_service(&id);
                let _ = tx.send((id, addr, hit));
            });
        }
        drop(tx);
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                tracing::warn!("WinRT: GATT probe budget exhausted, continuing");
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok((id, addr, true)) => {
                    tracing::info!("WinRT: GATT probe identified Mocopi at {addr:012X}");
                    let name = refresh_device_info(&id)
                        .map(|(n, _)| n)
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| format!("Mocopi ({})", u64_to_address(addr)));
                    matched.push(Candidate {
                        name,
                        paired: true,
                        addr,
                    });
                }
                Ok((_, _, false)) => {}
                Err(_) => break,
            }
        }
    }

    let mut out = Vec::new();
    for c in matched {
        let rssi = rssi_by_address.get(&c.addr).copied().unwrap_or(0);
        tracing::info!(
            "WinRT: Mocopi {} ({:012X}) RSSI {} dBm{}",
            c.name,
            c.addr,
            rssi,
            if rssi == 0 {
                " (no matching advertisement)"
            } else {
                ""
            }
        );
        let mut device = DeviceInfo::new(u64_to_address(c.addr), c.name, rssi);
        device.paired = c.paired;
        out.push(device);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    tracing::info!("WinRT: scan complete, found {} Mocopi device(s)", out.len());
    Ok(out)
}

pub(crate) async fn scan(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    // The blocking scan sleeps for (almost) the full budget while the
    // watcher listens, so the outer timeout needs headroom on top.
    let outer = timeout + Duration::from_secs(5);
    tokio::time::timeout(
        outer,
        tokio::task::spawn_blocking(move || blocking_scan_full(timeout)),
    )
    .await
    .map_err(|_| BleError::ConnectionTimeout)?
    .map_err(|e| BleError::ConnectionFailed {
        reason: format!("scan task failed: {e}"),
    })?
}

/// Cached service discovery: fast, no radio traffic. Used by the scan
/// GATT probe where speed matters more than completeness.
fn blocking_get_services_cached(device: &BluetoothLEDevice) -> Result<Vec<GattDeviceService>> {
    let result = device.GetGattServicesAsync().and_then(|op| op.get())?;
    let status = result.Status()?;
    if status != GattCommunicationStatus::Success {
        return Err(gatt_status_error(status, "GetGattServices"));
    }
    vector_view_to_vec(&result.Services()?)
}

/// Map a non-success GATT status to an error. AccessDenied typically
/// means the tracker wants to be paired first (Windows Settings →
/// Bluetooth → pair), so surface that hint for the GUI instead of a
/// cryptic status code.
fn gatt_status_error(status: GattCommunicationStatus, op: &str) -> BleError {
    if status == GattCommunicationStatus::AccessDenied {
        return BleError::ConnectionFailed {
            reason: format!(
                "{op} denied by device (AccessDenied). Pair the tracker in Windows Bluetooth settings, then retry."
            ),
        };
    }
    BleError::ConnectionFailed {
        reason: format!("{op} failed: {status:?}"),
    }
}

/// Uncached discovery: forces fresh GATT database reads over the air.
/// Slower, but required for connect — cached results are a notorious
/// source of "service not found" for devices Windows has only seen
/// via advertisements or stale pairings.
///
/// Retries on empty results: `GetGattServicesWithCacheModeAsync` can
/// report `Success` with an empty list when the link was just
/// (re-)established. Falls back to the cached list before giving up.
fn blocking_get_services(device: &BluetoothLEDevice) -> Result<Vec<GattDeviceService>> {
    // `Unreachable` right after (re-)connect is usually transient (link
    // still coming up), so every failure mode retries before giving up.
    let mut last_err: Option<BleError> = None;
    for attempt in 1..=4 {
        match device
            .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
            .and_then(|op| op.get())
        {
            Ok(result) => {
                let status = result
                    .Status()
                    .unwrap_or(GattCommunicationStatus::ProtocolError);
                if status == GattCommunicationStatus::Success {
                    let services = result
                        .Services()
                        .ok()
                        .and_then(|v| vector_view_to_vec(&v).ok())
                        .unwrap_or_default();
                    if !services.is_empty() {
                        // Top up with cached entries: cheap insurance
                        // against partial uncached results.
                        let services = match blocking_get_services_cached(device) {
                            Ok(cached) => merge_by_uuid(services, cached, |s| s.Uuid().ok()),
                            Err(_) => services,
                        };
                        log_service_uuids(&services);
                        return Ok(services);
                    }
                    tracing::debug!("WinRT: empty service list (attempt {attempt}/4), retrying");
                } else {
                    let e = gatt_status_error(status, "GetGattServices");
                    tracing::debug!("WinRT: {e} (attempt {attempt}/4), retrying");
                    last_err = Some(e);
                }
            }
            Err(e) => {
                tracing::debug!("WinRT: GetGattServices error (attempt {attempt}/4): {e:?}");
                last_err = Some(BleError::ConnectionFailed {
                    reason: format!("GetGattServices failed: {e:?}"),
                });
            }
        }
        if attempt < 4 {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    // Uncached discovery kept failing: fall back to whatever Windows
    // has cached (stale beats nothing).
    if let Ok(cached) = blocking_get_services_cached(device) {
        if !cached.is_empty() {
            tracing::warn!("WinRT: using cached GATT services after failed discovery");
            log_service_uuids(&cached);
            return Ok(cached);
        }
    }
    Err(last_err.unwrap_or(BleError::ConnectionFailed {
        reason: "GATT service discovery returned no services after retries \
                 (move the tracker closer and re-pair it in Windows Bluetooth settings)"
            .to_string(),
    }))
}

/// Union two discovery results by UUID (fresh entries win). Either side
/// can come back partial on a flaky link; the union still yields a
/// complete database.
fn merge_by_uuid<T, F>(mut primary: Vec<T>, secondary: Vec<T>, mut key: F) -> Vec<T>
where
    F: FnMut(&T) -> Option<GUID>,
{
    for item in secondary {
        let k = key(&item);
        if !primary.iter().any(|e| key(e) == k) {
            primary.push(item);
        }
    }
    primary
}

fn log_service_uuids(services: &[GattDeviceService]) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        let uuids: Vec<String> = services
            .iter()
            .filter_map(|s| s.Uuid().ok().map(|u| format!("{u:?}")))
            .collect();
        tracing::debug!("WinRT: device exposes services: {}", uuids.join(", "));
    }
}

fn blocking_get_characteristics(service: &GattDeviceService) -> Result<Vec<GattCharacteristic>> {
    let mut last_err: Option<BleError> = None;
    for attempt in 1..=3 {
        match service
            .GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)
            .and_then(|op| op.get())
        {
            Ok(result) => {
                let status = result
                    .Status()
                    .unwrap_or(GattCommunicationStatus::ProtocolError);
                if status == GattCommunicationStatus::Success {
                    let chars = result
                        .Characteristics()
                        .ok()
                        .and_then(|v| vector_view_to_vec(&v).ok())
                        .unwrap_or_default();
                    if !chars.is_empty() {
                        return Ok(chars);
                    }
                    tracing::debug!(
                        "WinRT: empty characteristic list (attempt {attempt}/3), retrying"
                    );
                    last_err = None;
                } else {
                    let e = gatt_status_error(status, "GetCharacteristics");
                    tracing::debug!("WinRT: {e} (attempt {attempt}/3), retrying");
                    last_err = Some(e);
                }
            }
            Err(e) => {
                tracing::debug!("WinRT: GetCharacteristics error (attempt {attempt}/3): {e:?}");
                last_err = Some(BleError::ConnectionFailed {
                    reason: format!("GetCharacteristics failed: {e:?}"),
                });
            }
        }
        if attempt < 3 {
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    // Success-but-empty maps to CharacteristicNotFound in the callers;
    // surface the last transport error only if discovery never succeeded.
    match last_err {
        Some(e) => Err(e),
        None => Ok(Vec::new()),
    }
}

fn find_service_by_uuid(
    services: &[GattDeviceService],
    uuid_str: &str,
) -> Result<GattDeviceService> {
    let want = ble_guid(uuid_str)?;
    for svc in services {
        if svc.Uuid()? == want {
            return Ok(svc.clone());
        }
    }
    Err(BleError::ServiceNotFound {
        uuid: uuid_str.to_string(),
    })
}

fn find_characteristic_by_uuid(
    chars: &[GattCharacteristic],
    uuid_str: &str,
) -> Result<GattCharacteristic> {
    let want = ble_guid(uuid_str)?;
    for ch in chars {
        if ch.Uuid()? == want {
            return Ok(ch.clone());
        }
    }
    Err(BleError::CharacteristicNotFound {
        uuid: uuid_str.to_string(),
    })
}

/// Whether notifications/indications can be enabled on this characteristic.
/// Current firmware funnels everything through one notify characteristic;
/// blindly enabling notify on the write-only CMD characteristic would
/// fail the whole connection with `NotifyFailed`.
fn supports_notify(ch: &GattCharacteristic) -> bool {
    ch.CharacteristicProperties()
        .map(|p| {
            p.contains(GattCharacteristicProperties::Notify)
                || p.contains(GattCharacteristicProperties::Indicate)
        })
        .unwrap_or(false)
}

fn same_characteristic(a: &GattCharacteristic, b: &GattCharacteristic) -> bool {
    match (a.AttributeHandle(), b.AttributeHandle()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Standard SIG services (generic access/gap/battery/…) carry no Mocopi
/// data; auxiliary subscriptions skip them to avoid noise.
fn is_standard_service(svc: &GattDeviceService) -> bool {
    svc.Uuid()
        .map(|u| format!("{u:?}").starts_with("0000180"))
        .unwrap_or(false)
}

/// Forward helper shared by the auxiliary stream subscriptions: decode
/// and pass through (IMU, battery, status — whatever the firmware sends
/// on that pipe). Unknown layouts degrade to `Unknown` packets in the
/// logs instead of silence.
fn forward_decoded(raw_tx: &mpsc::UnboundedSender<DecodedPacket>, bytes: Vec<u8>) {
    let packet = match decode_packet(&bytes, None) {
        Ok(p) => p,
        Err(_) => DecodedPacket::Unknown {
            packet_type: bytes.first().copied().unwrap_or(0),
            data: bytes,
        },
    };
    let _ = raw_tx.send(packet);
}

fn blocking_write_command(ch: &GattCharacteristic, cmd: &[u8]) -> Result<()> {
    let buffer = vec_to_buffer(cmd)?;
    let status = ch
        .WriteValueWithOptionAsync(&buffer, GattWriteOption::WriteWithResponse)
        .and_then(|op| op.get())?;
    if status != GattCommunicationStatus::Success {
        return Err(BleError::WriteFailed {
            reason: format!("write {cmd:02X?} failed: {status:?}"),
        });
    }
    Ok(())
}

fn blocking_enable_notify(ch: &GattCharacteristic) -> Result<()> {
    let status = ch
        .WriteClientCharacteristicConfigurationDescriptorAsync(
            GattClientCharacteristicConfigurationDescriptorValue::Notify,
        )
        .and_then(|op| op.get())?;
    if status != GattCommunicationStatus::Success {
        return Err(BleError::NotifyFailed {
            reason: format!("enable notify failed: {status:?}"),
        });
    }
    Ok(())
}

fn blocking_read_firmware(device: &BluetoothLEDevice) -> Option<String> {
    let services = blocking_get_services(device).ok()?;
    let svc = find_service_by_uuid(&services, "0000180a-0000-1000-8000-00805f9b34fb").ok()?;
    let chars = blocking_get_characteristics(&svc).ok()?;
    let ch = find_characteristic_by_uuid(&chars, "00002a26-0000-1000-8000-00805f9b34fb").ok()?;
    let result = ch.ReadValueAsync().and_then(|op| op.get()).ok()?;
    if result.Status().ok()? != GattCommunicationStatus::Success {
        return None;
    }
    let bytes = buffer_to_vec(&result.Value().ok()?).ok()?;
    let s = String::from_utf8_lossy(&bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

struct BringUp {
    device: BluetoothLEDevice,
    imu_char: GattCharacteristic,
    cmd_char: GattCharacteristic,
    imu_token: EventRegistrationToken,
    cmd_token: Option<EventRegistrationToken>,
    aux_tokens: Vec<(GattCharacteristic, EventRegistrationToken)>,
    last_imu_at_ms: Arc<AtomicU64>,
}

/// Live WinRT connection. Holds the device, characteristics and event
/// tokens so notifications keep flowing. `cmd_token` is `None` when the
/// command characteristic carries no notifications of its own (current
/// firmware funnels command responses through the IMU characteristic).
pub struct WinRtConnection {
    device: BluetoothLEDevice,
    imu_char: GattCharacteristic,
    cmd_char: GattCharacteristic,
    imu_token: EventRegistrationToken,
    cmd_token: Option<EventRegistrationToken>,
    aux_tokens: Vec<(GattCharacteristic, EventRegistrationToken)>,
    forwarder: tokio::task::JoinHandle<()>,
    last_imu_at_ms: Arc<AtomicU64>,
}

fn remove_value_changed(ch: &GattCharacteristic, token: Option<EventRegistrationToken>) {
    if let Some(token) = token {
        let _ = ch.RemoveValueChanged(token);
    }
}

impl Drop for WinRtConnection {
    fn drop(&mut self) {
        let _ = self.imu_char.RemoveValueChanged(self.imu_token);
        remove_value_changed(&self.cmd_char, self.cmd_token);
        for (ch, token) in &self.aux_tokens {
            let _ = ch.RemoveValueChanged(*token);
        }
        self.forwarder.abort();
    }
}

impl WinRtConnection {
    pub fn disconnect(mut self) {
        let _ = self.imu_char.RemoveValueChanged(self.imu_token);
        remove_value_changed(&self.cmd_char, self.cmd_token);
        for (ch, token) in self.aux_tokens.drain(..) {
            let _ = ch.RemoveValueChanged(token);
        }
        self.forwarder.abort();
    }

    pub fn device_name(&self) -> String {
        self.device
            .Name()
            .map(|n| n.to_string())
            .unwrap_or_default()
    }

    pub fn is_connected(&self) -> bool {
        let os_connected = self
            .device
            .ConnectionStatus()
            .map(|s| s == BluetoothConnectionStatus::Connected)
            .unwrap_or(false);
        if os_connected {
            return true;
        }
        // WinRT's device-level state can briefly lag the live GATT stream.
        // A recent IMU notification is stronger evidence that it is usable.
        let last = self.last_imu_at_ms.load(Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        last != 0 && now.saturating_sub(last) <= 5_000
    }

    pub fn has_tracking_data(&self) -> bool {
        self.last_imu_at_ms.load(Ordering::Relaxed) != 0
    }
}

pub(crate) async fn connect(
    address: &str,
    timeout: Duration,
    device_id: Uuid,
    packet_tx: broadcast::Sender<(Uuid, DecodedPacket)>,
) -> Result<(WinRtConnection, Option<String>)> {
    let addr_u64 = address_to_u64(address)?;
    let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<DecodedPacket>();

    let bring_up = tokio::task::spawn_blocking(move || -> Result<BringUp> {
        let device = BluetoothLEDevice::FromBluetoothAddressAsync(addr_u64)
            .and_then(|op| op.get())
            .map_err(|_| BleError::DeviceNotFound {
                identifier: format!("{addr_u64:012X}"),
            })?;
        // Uncached discovery: cached GATT lists are a notorious source
        // of "service not found" for devices Windows only saw via
        // advertisements or stale pairings.
        let services = blocking_get_services(&device)?;
        let cmd_svc = find_service_by_uuid(&services, CMD_SERVICE_UUID)?;
        let cmd_chars = blocking_get_characteristics(&cmd_svc)?;
        let cmd_char = find_characteristic_by_uuid(&cmd_chars, CMD_CHARACTERISTIC_UUID)?;
        // IMU stream resolution (verified against real firmware):
        // 1. Spec layout: characteristic FFF1 inside service FFF0.
        // 2. Current firmware: no FFF0 service at all; the stream runs
        //    over notify-capable FF03 inside the CMD service FF00
        //    (which also carries command responses on the same pipe).
        let imu_char = match find_service_by_uuid(&services, IMU_SERVICE_UUID) {
            Ok(imu_svc) => {
                let imu_chars = blocking_get_characteristics(&imu_svc)?;
                match find_characteristic_by_uuid(&imu_chars, IMU_CHARACTERISTIC_UUID) {
                    Ok(c) => c,
                    Err(_) => {
                        tracing::warn!(
                            "WinRT: {IMU_CHARACTERISTIC_UUID} missing in {IMU_SERVICE_UUID}, trying {IMU_FALLBACK_CHARACTERISTIC_UUID} in CMD service"
                        );
                        find_characteristic_by_uuid(&cmd_chars, IMU_FALLBACK_CHARACTERISTIC_UUID)?
                    }
                }
            }
            Err(_) => {
                // Normalfall aktueller Firmware (kein FFF0-Service):
                // Stream läuft über FF03 im CMD-Service.
                tracing::info!(
                    "WinRT: IMU service {IMU_SERVICE_UUID} missing, trying {IMU_FALLBACK_CHARACTERISTIC_UUID} in CMD service"
                );
                find_characteristic_by_uuid(&cmd_chars, IMU_FALLBACK_CHARACTERISTIC_UUID)?
            }
        };
        tracing::info!(
            "WinRT: IMU stream on {:?}, CMD on {:?}",
            imu_char.Uuid(),
            cmd_char.Uuid()
        );

        let tx_imu = raw_tx.clone();
        let last_imu_at_ms = Arc::new(AtomicU64::new(0));
        let notify_clock = last_imu_at_ms.clone();
        let imu_token = imu_char.ValueChanged(&TypedEventHandler::new(
            move |_sender: &Option<GattCharacteristic>,
                  args: &Option<GattValueChangedEventArgs>| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };
                let Ok(buffer) = args.CharacteristicValue() else {
                    return Ok(());
                };
                let Ok(bytes) = buffer_to_vec(&buffer) else {
                    return Ok(());
                };
                let packet = match decode_packet(&bytes, None) {
                    Ok(DecodedPacket::Imu(p)) => DecodedPacket::Imu(p),
                    Ok(other) => other,
                    Err(_) => DecodedPacket::Unknown {
                        packet_type: bytes.first().copied().unwrap_or(0),
                        data: bytes,
                    },
                };
                if matches!(&packet, DecodedPacket::Imu(_)) {
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    notify_clock.store(now_ms, Ordering::Relaxed);
                }
                let _ = tx_imu.send(packet);
                Ok(())
            },
        ))?;

        // The CMD characteristic only gets its own notification
        // subscription when it is a *different*, notify-capable
        // characteristic. On current firmware command responses arrive
        // on the shared IMU pipe, whose handler forwards every decoded
        // packet (IMU, battery, status) already.
        let cmd_token = if !same_characteristic(&cmd_char, &imu_char) && supports_notify(&cmd_char)
        {
            let tx_cmd = raw_tx.clone();
            Some(cmd_char.ValueChanged(&TypedEventHandler::new(
                move |_sender: &Option<GattCharacteristic>,
                      args: &Option<GattValueChangedEventArgs>| {
                    let Some(args) = args.as_ref() else {
                        return Ok(());
                    };
                    let Ok(buffer) = args.CharacteristicValue() else {
                        return Ok(());
                    };
                    let Ok(bytes) = buffer_to_vec(&buffer) else {
                        return Ok(());
                    };
                    let packet = match decode_packet(&bytes, None) {
                        Ok(p) => p,
                        Err(_) => DecodedPacket::Unknown {
                            packet_type: bytes.first().copied().unwrap_or(0),
                            data: bytes,
                        },
                    };
                    let _ = tx_cmd.send(packet);
                    Ok(())
                },
            ))?)
        } else {
            None
        };

        // Auxiliary stream subscriptions: notify-capable characteristics
        // in non-standard services, excluding the IMU/CMD pipes above.
        // Firmware layouts differ (verified: the 0x30 stream runs on a
        // vendor characteristic outside FF00/FFF0); everything is routed
        // through the same decode path, so extra pipes only add data.
        // Best effort per pipe: one failing subscription never fails
        // the connection.
        let mut aux_tokens = Vec::new();
        for svc in &services {
            if aux_tokens.len() >= 4 || is_standard_service(svc) {
                continue;
            }
            let chars = match blocking_get_characteristics(svc) {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!("WinRT: aux characteristic walk failed: {e}");
                    continue;
                }
            };
            for ch in chars {
                if aux_tokens.len() >= 4
                    || same_characteristic(&ch, &imu_char)
                    || same_characteristic(&ch, &cmd_char)
                    || !supports_notify(&ch)
                {
                    continue;
                }
                let uuid = ch.Uuid().map(|u| format!("{u:?}")).unwrap_or_default();
                let tx_aux = raw_tx.clone();
                match ch.ValueChanged(&TypedEventHandler::new(
                    move |_sender: &Option<GattCharacteristic>,
                          args: &Option<GattValueChangedEventArgs>| {
                        let Some(args) = args.as_ref() else {
                            return Ok(());
                        };
                        let Ok(buffer) = args.CharacteristicValue() else {
                            return Ok(());
                        };
                        let Ok(bytes) = buffer_to_vec(&buffer) else {
                            return Ok(());
                        };
                        forward_decoded(&tx_aux, bytes);
                        Ok(())
                    },
                )) {
                    Ok(token) => match blocking_enable_notify(&ch) {
                        Ok(()) => {
                            tracing::info!("WinRT: subscribed aux stream {uuid}");
                            aux_tokens.push((ch.clone(), token));
                        }
                        Err(e) => {
                            tracing::debug!("WinRT: aux notify enable failed for {uuid}: {e}");
                            let _ = ch.RemoveValueChanged(token);
                        }
                    },
                    Err(e) => {
                        tracing::debug!("WinRT: aux subscribe failed for {uuid}: {e:?}");
                    }
                }
            }
        }

        blocking_enable_notify(&imu_char)?;
        if cmd_token.is_some() {
            blocking_enable_notify(&cmd_char)?;
        }
        blocking_write_command(&cmd_char, CMD_START_STREAMING)?;

        Ok(BringUp {
            device,
            imu_char,
            cmd_char,
            imu_token,
            cmd_token,
            aux_tokens,
            last_imu_at_ms,
        })
    });

    let bring_up: BringUp = tokio::time::timeout(timeout, bring_up)
        .await
        .map_err(|_| BleError::ConnectionTimeout)?
        .map_err(|e| BleError::ConnectionFailed {
            reason: format!("connect task failed: {e}"),
        })??;

    let forwarder = tokio::spawn(async move {
        while let Some(packet) = raw_rx.recv().await {
            let _ = packet_tx.send((device_id, packet));
        }
    });

    let fw = {
        tokio::task::spawn_blocking(move || {
            BluetoothLEDevice::FromBluetoothAddressAsync(addr_u64)
                .and_then(|op| op.get())
                .ok()
                .and_then(|dev| blocking_read_firmware(&dev))
        })
        .await
        .ok()
        .flatten()
    };

    Ok((
        WinRtConnection {
            device: bring_up.device,
            imu_char: bring_up.imu_char,
            cmd_char: bring_up.cmd_char,
            imu_token: bring_up.imu_token,
            cmd_token: bring_up.cmd_token,
            aux_tokens: bring_up.aux_tokens,
            forwarder,
            last_imu_at_ms: bring_up.last_imu_at_ms,
        },
        fw,
    ))
}

pub(crate) async fn write_command(
    conn_device_addr: u64,
    cmd: Vec<u8>,
    timeout: Duration,
) -> Result<()> {
    // Re-resolve the device for one-shot writes (battery poll). The live
    // notification path stays on the WinRtConnection; this helper is only
    // for explicit commands where we don't want to hold WinRT objects
    // across await points.
    tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(move || -> Result<()> {
            let device = BluetoothLEDevice::FromBluetoothAddressAsync(conn_device_addr)
                .and_then(|op| op.get())
                .map_err(|_| BleError::DeviceNotFound {
                    identifier: format!("{conn_device_addr:012X}"),
                })?;
            let services = blocking_get_services(&device)?;
            let cmd_svc = find_service_by_uuid(&services, CMD_SERVICE_UUID)?;
            let chars = blocking_get_characteristics(&cmd_svc)?;
            let ch = find_characteristic_by_uuid(&chars, CMD_CHARACTERISTIC_UUID)?;
            blocking_write_command(&ch, &cmd)
        }),
    )
    .await
    .map_err(|_| BleError::ConnectionTimeout)?
    .map_err(|e| BleError::ConnectionFailed {
        reason: format!("write task failed: {e}"),
    })?
}
