use crate::error::FfiError;
use configuration::AppConfig;
use moslime_core::{CoreCommand, CoreEvent, TrackerManager};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock, RwLock,
};
use tokio::sync::broadcast;
use tracing::error;
use uuid::Uuid;

static MANAGER: RwLock<Option<Arc<TrackerManager>>> = RwLock::new(None);
static EVENT_RX: Mutex<Option<broadcast::Receiver<CoreEvent>>> = Mutex::new(None);
static COMMAND_TX: Mutex<Option<tokio::sync::mpsc::Sender<CoreCommand>>> = Mutex::new(None);
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
/// Zähler für gedroppte Events (`Lagged`), wenn die GUI langsamer pollt
/// als der Core sendet. Wird in Diagnostics + `moslime_get_event_stats`
/// exponiert, damit "GUI hängt hinterher" diagnosierbar ist.
static EVENT_LAGGED: AtomicU64 = AtomicU64::new(0);

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
}

fn manager() -> Option<Arc<TrackerManager>> {
    MANAGER.read().ok().and_then(|g| g.clone())
}

fn command_tx() -> Option<tokio::sync::mpsc::Sender<CoreCommand>> {
    COMMAND_TX.lock().ok().and_then(|g| g.clone())
}

fn manager_log_level(manager: &TrackerManager) -> String {
    get_runtime()
        .block_on(manager.get_config())
        .general
        .log_level
}

#[no_mangle]
pub unsafe extern "C" fn moslime_init(config_json: *const c_char) -> c_int {
    // Ensure config/log directories exist BEFORE any other operations
    moslime_core::logging::ensure_dirs();
    // A Rust panic must end up in the logs, never vanish silently.
    moslime_core::logging::init_panic_hook();

    // Re-Init nach Shutdown erlauben: Wenn bereits ein Manager lebt,
    // AlreadyInitialized zurückgeben, sonst neu aufbauen.
    if manager().is_some() {
        return FfiError::AlreadyInitialized as c_int;
    }
    let config_str = unsafe {
        if config_json.is_null() {
            "{}"
        } else {
            match CStr::from_ptr(config_json).to_str() {
                Ok(s) => s,
                Err(_) => return FfiError::InvalidInput as c_int,
            }
        }
    };

    let config: AppConfig = serde_json::from_str(config_str).unwrap_or_default();

    let rt = get_runtime();
    let ble_manager = Arc::new(mocopi_ble_windows::BleManager::new());
    let (manager, event_rx, cmd_tx) = TrackerManager::new(ble_manager, config);
    let manager = Arc::new(manager);
    moslime_core::init_logging(&manager.logs(), &manager_log_level(&manager));
    rt.spawn({
        let manager = manager.clone();
        async move {
            if let Err(e) = manager.run().await {
                error!("TrackerManager run error: {e}");
            }
        }
    });

    if MANAGER.write().map(|mut g| g.replace(manager)).is_err() {
        return FfiError::InternalError as c_int;
    }
    if COMMAND_TX.lock().map(|mut g| g.replace(cmd_tx)).is_err() {
        return FfiError::InternalError as c_int;
    }
    match EVENT_RX.lock() {
        Ok(mut guard) => {
            *guard = Some(event_rx);
        }
        Err(_) => return FfiError::InternalError as c_int,
    }
    FfiError::Success as c_int
}

#[no_mangle]
pub extern "C" fn moslime_shutdown() -> c_int {
    let Some(m) = manager() else {
        return FfiError::NotInitialized as c_int;
    };
    get_runtime().block_on(async {
        let _ = m.shutdown().await;
    });
    // State clearen, damit ein zweites `moslime_init` im selben Prozess
    // funktioniert (z.B. nach Settings-Reset oder GUI-Neustart ohne
    // Prozess-Neustart). Runtime lebt weiter (OnceLock, prozessweit).
    if let Ok(mut guard) = EVENT_RX.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = COMMAND_TX.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = MANAGER.write() {
        *guard = None;
    }
    FfiError::Success as c_int
}

fn parse_device_id(ptr: *const c_char) -> std::result::Result<Uuid, FfiError> {
    if ptr.is_null() {
        return Err(FfiError::InvalidInput);
    }
    let s = unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map_err(|_| FfiError::InvalidInput)?
    };
    Uuid::parse_str(s).map_err(|_| FfiError::InvalidInput)
}

fn parse_str(ptr: *const c_char) -> std::result::Result<String, FfiError> {
    if ptr.is_null() {
        return Err(FfiError::InvalidInput);
    }
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| FfiError::InvalidInput)
    }
}

#[no_mangle]
pub extern "C" fn moslime_scan_devices() -> c_int {
    tracing::info!("FFI: moslime_scan_devices called");
    send_command(CoreCommand::ScanDevices)
}

#[no_mangle]
pub extern "C" fn moslime_connect_device(device_id_str: *const c_char) -> c_int {
    match parse_device_id(device_id_str) {
        Ok(id) => send_command(CoreCommand::ConnectDevice(id)),
        Err(e) => e as c_int,
    }
}

#[no_mangle]
pub extern "C" fn moslime_disconnect_device(device_id_str: *const c_char) -> c_int {
    match parse_device_id(device_id_str) {
        Ok(id) => send_command(CoreCommand::DisconnectDevice(id)),
        Err(e) => e as c_int,
    }
}

#[no_mangle]
pub extern "C" fn moslime_assign_tracker(device_id_str: *const c_char, role: c_int) -> c_int {
    let device_id = match parse_device_id(device_id_str) {
        Ok(id) => id,
        Err(e) => return e as c_int,
    };
    let role_u8: u8 = match role.try_into() {
        Ok(v) => v,
        Err(_) => return FfiError::InvalidInput as c_int,
    };
    match tracking_core::TrackerRole::try_from(role_u8) {
        Ok(r) => send_command(CoreCommand::AssignTracker(device_id, r)),
        Err(_) => FfiError::InvalidInput as c_int,
    }
}

/// Aktive Pose fürs Tracking setzen (manueller Override, pausiert Auto 30s).
#[no_mangle]
pub unsafe extern "C" fn moslime_set_active_pose(pose_str: *const c_char) -> c_int {
    let pose_s = match parse_str(pose_str) {
        Ok(s) => s,
        Err(e) => return e as c_int,
    };
    let pose: tracking_core::CalibrationPose = match pose_s.parse() {
        Ok(p) => p,
        Err(_) => return FfiError::InvalidInput as c_int,
    };
    send_command(CoreCommand::SetActivePose(pose))
}

/// Auto-Pose an/aus (1/0).
#[no_mangle]
pub extern "C" fn moslime_set_auto_pose(enabled: c_int) -> c_int {
    send_command(CoreCommand::SetAutoPose(enabled != 0))
}

/// Mount-Flip pro Tracker (1 = 180° Yaw-Korrektur, 0 = normal).
#[no_mangle]
pub extern "C" fn moslime_set_mount_flip(device_id_str: *const c_char, flip: c_int) -> c_int {
    let id = match parse_device_id(device_id_str) {
        Ok(id) => id,
        Err(e) => return e as c_int,
    };
    send_command(CoreCommand::SetMountFlip(id, flip != 0))
}

/// 1 = Scan läuft, 0 = idle, -1 = ohne Core.
#[no_mangle]
pub extern "C" fn moslime_is_scanning() -> c_int {
    let Some(m) = manager() else {
        return -1;
    };
    if get_runtime().block_on(m.is_scanning()) {
        1
    } else {
        0
    }
}

/// Skeleton-Maße als JSON (`tracking_core::SkeletonConfig`). Ungültig -> InvalidInput.
#[no_mangle]
pub unsafe extern "C" fn moslime_set_skeleton(config_json: *const c_char) -> c_int {
    let s = match parse_str(config_json) {
        Ok(v) => v,
        Err(e) => return e as c_int,
    };
    match serde_json::from_str::<tracking_core::SkeletonConfig>(&s) {
        Ok(cfg) => {
            if cfg.validate().is_err() {
                return FfiError::InvalidInput as c_int;
            }
            send_command(CoreCommand::SetSkeleton(Box::new(cfg)))
        }
        Err(_) => FfiError::InvalidInput as c_int,
    }
}

/// Aktuelle Haltungserkennung als JSON.
#[no_mangle]
pub extern "C" fn moslime_get_pose_status() -> *mut c_char {
    use serde::Serialize;
    #[derive(Serialize)]
    struct PoseStatus {
        active_pose: String,
        auto_enabled: bool,
        confidence: f32,
    }
    let Some(manager) = manager() else {
        return ptr::null_mut();
    };
    let runtime = get_runtime();
    let status = PoseStatus {
        active_pose: runtime.block_on(manager.active_pose()).name().to_string(),
        auto_enabled: runtime.block_on(manager.auto_pose_enabled()),
        confidence: runtime.block_on(manager.auto_pose_confidence()),
    };
    match serde_json::to_string(&status) {
        Ok(json) => string_to_raw(json),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn moslime_start_streaming() -> c_int {
    send_command(CoreCommand::StartStreaming)
}

#[no_mangle]
pub extern "C" fn moslime_stop_streaming() -> c_int {
    send_command(CoreCommand::StopStreaming)
}

#[no_mangle]
pub unsafe extern "C" fn moslime_set_slimevr_address(ip: *const c_char, port: c_int) -> c_int {
    let ip_str = match parse_str(ip) {
        Ok(s) => s,
        Err(e) => return e as c_int,
    };
    let port_u16: u16 = match port.try_into() {
        Ok(v) => v,
        Err(_) => return FfiError::InvalidInput as c_int,
    };
    send_command(CoreCommand::SetSlimeVRAddress(ip_str, port_u16))
}

#[no_mangle]
pub unsafe extern "C" fn moslime_set_config(config_json: *const c_char) -> c_int {
    let config_str = match parse_str(config_json) {
        Ok(s) => s,
        Err(e) => return e as c_int,
    };
    match serde_json::from_str::<AppConfig>(&config_str) {
        Ok(c) => send_command(CoreCommand::SetConfig(Box::new(c))),
        Err(_) => FfiError::SerializationError as c_int,
    }
}

/// Persist only the UI language, preserving the rest of the live config.
#[no_mangle]
pub unsafe extern "C" fn moslime_set_language(language: *const c_char) -> c_int {
    let language = match parse_str(language) {
        Ok(value) => value,
        Err(error) => return error as c_int,
    };
    if !matches!(language.as_str(), "en" | "de" | "ja" | "zh-CN") {
        return FfiError::InvalidInput as c_int;
    }
    send_command(CoreCommand::SetLanguage(language))
}

/// Load the on-disk config (`%APPDATA%/Mocoslime/config.json`, created
/// with defaults on first run) into the running core. Called once by the
/// GUI after `moslime_init` so settings and role assignments survive
/// restarts. Returns 0 on success, 1 (`NotInitialized`) without a core,
/// 5 (`InternalError`) when the file cannot be read/parsed/applied.
#[no_mangle]
pub extern "C" fn moslime_load_config() -> c_int {
    let Some(m) = manager() else {
        return FfiError::NotInitialized as c_int;
    };
    let path = AppConfig::default_config_path();
    match AppConfig::load(&path) {
        Ok(config) => get_runtime().block_on(async {
            match m.update_config_public(config).await {
                Ok(()) => FfiError::Success as c_int,
                Err(e) => {
                    error!("load_config apply failed: {e}");
                    FfiError::InternalError as c_int
                }
            }
        }),
        Err(e) => {
            error!("load_config failed ({}): {e}", path.display());
            FfiError::InternalError as c_int
        }
    }
}

fn string_to_raw(json: String) -> *mut c_char {
    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn moslime_get_config() -> *mut c_char {
    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    let config = get_runtime().block_on(m.get_config());
    match serde_json::to_string(&config) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn moslime_get_devices() -> *mut c_char {
    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    let devices = get_runtime().block_on(m.get_all_trackers());
    match serde_json::to_string(&devices) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

/// Single-device status as JSON (`TrackerStatus`), or the JSON literal
/// `null` when the id is unknown (Dart treats both as "no device").
/// The returned pointer must be freed with `moslime_free_string`.
#[no_mangle]
pub extern "C" fn moslime_get_device(device_id_str: *const c_char) -> *mut c_char {
    let device_id = match parse_device_id(device_id_str) {
        Ok(id) => id,
        Err(_) => return ptr::null_mut(),
    };
    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    let status = get_runtime().block_on(m.get_tracker_status(&device_id));
    match serde_json::to_string(&status) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

// The returned pointer must be freed with moslime_free_string.
#[no_mangle]
pub extern "C" fn moslime_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

#[no_mangle]
pub extern "C" fn moslime_poll_event() -> *mut c_char {
    let Ok(mut guard) = EVENT_RX.lock() else {
        return ptr::null_mut();
    };
    let Some(rx) = guard.as_mut() else {
        return ptr::null_mut();
    };
    match rx.try_recv() {
        Ok(event) => match serde_json::to_string(&event) {
            Ok(j) => string_to_raw(j),
            Err(_) => ptr::null_mut(),
        },
        Err(broadcast::error::TryRecvError::Lagged(n)) => {
            EVENT_LAGGED.fetch_add(n as u64, Ordering::Relaxed);
            // Nach Lag sofort das neueste Event liefern statt null, damit
            // die GUI nicht einen Poll-Zyklus verliert.
            match rx.try_recv() {
                Ok(event) => match serde_json::to_string(&event) {
                    Ok(j) => string_to_raw(j),
                    Err(_) => ptr::null_mut(),
                },
                Err(_) => ptr::null_mut(),
            }
        }
        // Empty/Closed bedeuten "kein Event gerade".
        Err(_) => ptr::null_mut(),
    }
}

/// Event-Statistiken als JSON: `{lagged, api_version}`.
/// `lagged` = gedroppte Events durch langsames Pollen (kumulativ).
#[no_mangle]
pub extern "C" fn moslime_get_event_stats() -> *mut c_char {
    use serde::Serialize;

    #[derive(Serialize)]
    struct EventStats {
        lagged: u64,
        api_version: i32,
    }
    let stats = EventStats {
        lagged: EVENT_LAGGED.load(Ordering::Relaxed),
        api_version: moslime_api_version(),
    };
    match serde_json::to_string(&stats) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn moslime_get_version() -> *mut c_char {
    string_to_raw(env!("CARGO_PKG_VERSION").to_string())
}

/// Versioned FFI API number (docs/migration.md – bump on breaking changes).
/// v5: Kalibrier-Math (Mounting-Nullpunkt), Config v3.
#[no_mangle]
pub extern "C" fn moslime_api_version() -> c_int {
    5
}

/// # Safety
/// `level` must be null or a valid NUL-terminated string
/// (`TRACE|DEBUG|INFO|WARN|ERROR`, case-insensitive).
#[no_mangle]
pub unsafe extern "C" fn moslime_set_log_level(level: *const c_char) -> c_int {
    let level_str = if level.is_null() {
        "INFO".to_string()
    } else {
        match unsafe { CStr::from_ptr(level).to_str() } {
            Ok(s) => s.to_string(),
            Err(_) => return FfiError::InvalidInput as c_int,
        }
    };
    moslime_core::set_log_level(&level_str);
    // Persist so restarts keep the verbosity.
    if let Some(m) = manager() {
        let rt = get_runtime();
        rt.block_on(async {
            // Best effort: read-modify-write is racy by design here;
            // settings saves serialize through the same path.
            let mut cfg = m.get_config().await;
            cfg.general.log_level = level_str;
            let _ = m.update_config_public(cfg).await;
        });
    }
    FfiError::Success as c_int
}

/// Drain buffered log entries as a JSON array. Never blocks the caller.
#[no_mangle]
pub extern "C" fn moslime_poll_logs() -> *mut c_char {
    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    match serde_json::to_string(&m.logs().drain()) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

/// Support bundle: version, OS, Bluetooth adapter state, full config,
/// known trackers and the newest log entries — everything needed to
/// diagnose "findet keinen Tracker" from a single JSON document.
/// Non-draining (unlike [`moslime_poll_logs`]); the returned pointer
/// must be freed with `moslime_free_string`.
#[no_mangle]
pub extern "C" fn moslime_get_diagnostics() -> *mut c_char {
    use serde::Serialize;

    #[derive(Serialize)]
    struct Diagnostics {
        version: String,
        api_version: i32,
        os: String,
        arch: String,
        bluetooth_adapter_ok: bool,
        bluetooth_adapter_error: Option<String>,
        ble_config: mocopi_ble_windows::BleConfig,
        tracker_count: usize,
        trackers: Vec<mocopi_ble_windows::TrackerStatus>,
        config: configuration::AppConfig,
        recent_logs: Vec<moslime_core::logging::LogEntry>,
        log_dir: String,
        event_lagged: u64,
    }

    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    let rt = get_runtime();
    let config = rt.block_on(m.get_config());
    let trackers = rt.block_on(m.get_all_trackers());
    let (adapter_ok, adapter_error) = match mocopi_ble_windows::adapter_status() {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    let diagnostics = Diagnostics {
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: moslime_api_version(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        bluetooth_adapter_ok: adapter_ok,
        bluetooth_adapter_error: adapter_error,
        ble_config: rt.block_on(m.ble_config()),
        tracker_count: trackers.len(),
        trackers,
        config,
        recent_logs: m.logs().snapshot(200),
        log_dir: moslime_core::logging::log_dir().display().to_string(),
        event_lagged: EVENT_LAGGED.load(Ordering::Relaxed),
    };
    match serde_json::to_string_pretty(&diagnostics) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

/// # Safety
/// None (plain integer argument).
#[no_mangle]
pub extern "C" fn moslime_set_autostart(enabled: c_int) -> c_int {
    match configuration::set_autostart(enabled != 0) {
        Ok(()) => FfiError::Success as c_int,
        Err(e) => {
            error!("set_autostart failed: {e}");
            FfiError::InternalError as c_int
        }
    }
}

#[no_mangle]
pub extern "C" fn moslime_is_autostart() -> c_int {
    match configuration::is_autostart_enabled() {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(_) => -1,
    }
}

/// # Safety
/// `config_json` must be a valid NUL-terminated UTF-8 JSON string
/// matching `mocopi_ble_windows::BleConfig`.
#[no_mangle]
pub unsafe extern "C" fn moslime_set_bluetooth_config(config_json: *const c_char) -> c_int {
    let config_str = match parse_str(config_json) {
        Ok(s) => s,
        Err(e) => return e as c_int,
    };
    let ble_config: mocopi_ble_windows::BleConfig = match serde_json::from_str(&config_str) {
        Ok(c) => c,
        Err(_) => return FfiError::InvalidInput as c_int,
    };
    send_command(CoreCommand::SetBleConfig(Box::new(ble_config)))
}

/// SlimeVR-Zielstatus als JSON (`SlimevrStatus`): enabled, ip, port,
/// auto_discovered, packets_sent, errors.
#[no_mangle]
pub extern "C" fn moslime_get_slimevr_status() -> *mut c_char {
    let Some(m) = manager() else {
        return ptr::null_mut();
    };
    let status = get_runtime().block_on(m.get_slimevr_status());
    match serde_json::to_string(&status) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

/// SlimeVR-Handshakes für alle streamenden Tracker erneut senden
/// (z.B. nach Server-Neustart – ohne Handshake keine Sensoren).
#[no_mangle]
pub extern "C" fn moslime_resend_handshake() -> c_int {
    send_command(CoreCommand::ResendHandshake)
}

/// Leichter Adapter-Check als JSON: `{ok, error}`. Für die Dashboard-Karte,
/// ohne das schwere Diagnostics-Bundle zu ziehen.
#[no_mangle]
pub extern "C" fn moslime_get_adapter_status() -> *mut c_char {
    use serde::Serialize;

    #[derive(Serialize)]
    struct AdapterStatus {
        ok: bool,
        error: Option<String>,
    }
    let status = match mocopi_ble_windows::adapter_status() {
        Ok(()) => AdapterStatus {
            ok: true,
            error: None,
        },
        Err(e) => AdapterStatus {
            ok: false,
            error: Some(e.to_string()),
        },
    };
    match serde_json::to_string(&status) {
        Ok(j) => string_to_raw(j),
        Err(_) => ptr::null_mut(),
    }
}

fn send_command(cmd: CoreCommand) -> c_int {
    match command_tx() {
        Some(tx) => {
            if tx.try_send(cmd).is_ok() {
                FfiError::Success as c_int
            } else {
                FfiError::ChannelFull as c_int
            }
        }
        None => FfiError::NotInitialized as c_int,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_int;

    #[test]
    fn test_ffi_error_codes() {
        assert_eq!(FfiError::Success as c_int, 0);
        assert_eq!(FfiError::NotInitialized as c_int, 1);
        assert_eq!(FfiError::InvalidInput as c_int, 2);
    }

    #[test]
    fn test_parse_device_id_null() {
        assert_eq!(parse_device_id(ptr::null()), Err(FfiError::InvalidInput));
    }

    #[test]
    fn test_api_version_is_v5() {
        assert!(moslime_api_version() >= 5);
    }
}
