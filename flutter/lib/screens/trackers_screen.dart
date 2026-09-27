import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../widgets/localized_text.dart';
import '../state/app_state.dart';

class TrackersScreen extends StatefulWidget {
  const TrackersScreen({super.key});

  @override
  State<TrackersScreen> createState() => _TrackersScreenState();
}

class _TrackersScreenState extends State<TrackersScreen> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<AppState>().refresh();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        final trackers = appState.visibleTrackers;
        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: LocalizedText(
                          'Trackers (${trackers.length}/${appState.trackers.length})',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                      ),
                      ElevatedButton.icon(
                        icon: appState.bleState == 'Scanning...'
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2),
                              )
                            : const Icon(Icons.bluetooth_searching),
                        label: LocalizedText(appState.bleState == 'Scanning...'
                            ? 'Scanning...'
                            : 'Scan for Devices'),
                        onPressed: appState.bleState == 'Scanning...'
                            ? null
                            : () => appState.scanDevices(),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      Expanded(
                        child: LocalizedText(
                          'Min. Signal: ${appState.settings.bluetoothRssiMinDbm} dBm',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                      const LocalizedText('Hide weak signals'),
                      Switch(
                        value: appState.hideWeakDevices,
                        onChanged: (v) => appState.setHideWeakDevices(v),
                      ),
                    ],
                  ),
                  Slider(
                    min: -100,
                    max: -40,
                    divisions: 12,
                    value: appState.settings.bluetoothRssiMinDbm
                        .toDouble()
                        .clamp(-100, -40),
                    label: '${appState.settings.bluetoothRssiMinDbm} dBm',
                    onChanged: (v) => appState.updateSetting(
                        'bluetoothRssiMinDbm', v.round()),
                  ),
                ],
              ),
            ),
            Expanded(
              child: trackers.isEmpty
                  ? _buildEmptyState(context)
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      itemCount: trackers.length,
                      itemBuilder: (context, index) {
                        return _TrackerDetailCard(
                          tracker: trackers[index],
                          trackingSample: appState
                              .trackingSampleFor(trackers[index].deviceId),
                          estimatedPosition: appState
                              .estimatedPositionFor(trackers[index].deviceId),
                          dataRate: appState
                              .trackerDataRate(trackers[index].deviceId),
                          mountFlip: appState
                              .isMountFlip(trackers[index].bluetoothAddress),
                          onAssignRole: (role) => appState.assignTracker(
                              trackers[index].deviceId, role),
                          onConnect: () =>
                              appState.connectTracker(trackers[index].deviceId),
                          onDisconnect: () => appState
                              .disconnectTracker(trackers[index].deviceId),
                          onReconnect: () => appState
                              .reconnectTracker(trackers[index].deviceId),
                          onMountFlip: (flip) => appState.setMountFlip(
                              trackers[index].deviceId,
                              trackers[index].bluetoothAddress,
                              flip),
                        );
                      },
                    ),
            ),
          ],
        );
      },
    );
  }

  Widget _buildEmptyState(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const Icon(Icons.devices, size: 80, color: Colors.grey),
          const SizedBox(height: 24),
          LocalizedText(
            'No Trackers Found',
            style: Theme.of(context).textTheme.headlineMedium,
          ),
          const SizedBox(height: 12),
          LocalizedText(
            'Connect your Mocopi trackers via Bluetooth',
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: Colors.grey,
                ),
          ),
          const SizedBox(height: 24),
          ElevatedButton.icon(
            icon: context.watch<AppState>().bleState == 'Scanning...'
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.bluetooth_searching),
            label: LocalizedText(
                context.watch<AppState>().bleState == 'Scanning...'
                    ? 'Scanning...'
                    : 'Scan for Devices'),
            onPressed: context.watch<AppState>().bleState == 'Scanning...'
                ? null
                : () => context.read<AppState>().scanDevices(),
          ),
        ],
      ),
    );
  }
}

class _TrackerDetailCard extends StatelessWidget {
  final TrackerStatus tracker;
  final Map<String, dynamic>? trackingSample;
  final List<double>? estimatedPosition;
  final double dataRate;
  final bool mountFlip;
  final Function(String) onAssignRole;
  final VoidCallback onConnect;
  final VoidCallback onDisconnect;
  final VoidCallback onReconnect;
  final Function(bool) onMountFlip;

  const _TrackerDetailCard({
    required this.tracker,
    this.trackingSample,
    this.estimatedPosition,
    this.dataRate = 0,
    this.mountFlip = false,
    required this.onAssignRole,
    required this.onConnect,
    required this.onDisconnect,
    required this.onReconnect,
    required this.onMountFlip,
  });

  @override
  Widget build(BuildContext context) {
    final List<Widget> trailingChildren = [];
    if (tracker.battery != null) {
      trailingChildren.add(_BatteryIndicator(battery: tracker.battery!));
    }
    trailingChildren.add(
      LocalizedTooltip(
        message: tracker.rssi == 0
            ? 'No RSSI sample yet. Scan again while the tracker is powered on.'
            : 'Bluetooth signal strength',
        child: LocalizedText(
          tracker.rssi == 0 ? 'RSSI unknown' : '${tracker.rssi} dBm',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: _getRssiColor(tracker.rssi),
              ),
        ),
      ),
    );

    final List<Widget> actionButtons = [];
    if (!tracker.isConnected) {
      final retry = tracker.hasError || tracker.isReconnecting;
      actionButtons.add(
        ElevatedButton.icon(
          icon: const Icon(Icons.bluetooth),
          label: LocalizedText(retry ? 'Reconnect' : 'Connect'),
          onPressed: onConnect,
        ),
      );
    }
    actionButtons.add(
      LocalizedTooltip(
        message:
            'Disconnecting stops auto-reconnect in Mocoslime; the tracker itself stays powered on.',
        child: OutlinedButton.icon(
          icon: const Icon(Icons.power_settings_new),
          label: const LocalizedText('Disconnect'),
          onPressed: onDisconnect,
        ),
      ),
    );
    actionButtons.add(
      LocalizedTooltip(
        message: 'Disconnect and reconnect this tracker manually',
        child: OutlinedButton.icon(
          icon: const Icon(Icons.sync),
          label: const LocalizedText('Reconnect'),
          onPressed: tracker.isConnecting ? null : onReconnect,
        ),
      ),
    );

    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: ExpansionTile(
        leading: CircleAvatar(
          backgroundColor: _getStateColor(tracker.state),
          child: Icon(
            tracker.isConnected
                ? Icons.check_circle
                : Icons.radio_button_unchecked,
            color: Colors.white,
          ),
        ),
        title: LocalizedText(tracker.name,
            style: Theme.of(context).textTheme.titleLarge),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            LocalizedText('MAC: ${tracker.bluetoothAddress}'),
            LocalizedText('Role: ${tracker.assignedRole ?? 'Unassigned'}'),
            LocalizedText(
              _formatPacketAge(tracker.lastPacketAge),
              style: TextStyle(
                color: tracker.hasFreshTrackingData
                    ? Colors.green
                    : tracker.lastPacketAge == null
                        ? Colors.grey
                        : Colors.orange,
              ),
            ),
            if (tracker.state == 'Reconnecting')
              LocalizedText(
                'Reconnecting… Versuch ${tracker.reconnectAttempts} (5s→60s Backoff)${tracker.reconnectInMs != null ? ' • nächster in ${(tracker.reconnectInMs! / 1000).ceil()}s' : ''}',
                style: const TextStyle(color: Colors.orange),
              )
            else if (tracker.reconnectAttempts > 0 && tracker.hasError)
              LocalizedText(
                'Reconnect-Versuche: ${tracker.reconnectAttempts}/∞ (Auto läuft)${tracker.reconnectInMs != null ? ' • nächster in ${(tracker.reconnectInMs! / 1000).ceil()}s' : ''}',
                style: const TextStyle(color: Colors.orange),
              ),
          ],
        ),
        trailing: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: trailingChildren,
        ),
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _buildInfoRow('Device ID', tracker.deviceId),
                _buildInfoRow('Firmware', tracker.firmwareVersion ?? 'Unknown'),
                _buildInfoRow(
                    'Packets Received', tracker.packetCount.toString()),
                _buildInfoRow(
                    'Live Data Rate', '${dataRate.toStringAsFixed(1)} pkt/s'),
                _buildInfoRow('Movement (accel)',
                    _vectorText(trackingSample?['acceleration'])),
                _buildInfoRow(
                    'Orientation (quat)',
                    _vectorText(trackingSample?['quaternion'],
                        quaternion: true)),
                _buildInfoRow(
                  'Estimated Position (m)',
                  estimatedPosition == null
                      ? (tracker.assignedRole == null
                          ? 'Assign a role to estimate'
                          : 'Waiting for position sample')
                      : estimatedPosition!
                          .map((v) => v.toStringAsFixed(2))
                          .join(', '),
                ),
                _buildInfoRow(
                  'Tracking Data',
                  _formatPacketAge(tracker.lastPacketAge),
                  color: tracker.hasFreshTrackingData
                      ? Colors.green
                      : tracker.lastPacketAge == null
                          ? Colors.grey
                          : Colors.orange,
                ),
                _buildInfoRow(
                    'Reconnect Attempts', tracker.reconnectAttempts.toString()),
                if (tracker.error != null)
                  _buildInfoRow('Error', tracker.error!, color: Colors.red),
                const SizedBox(height: 16),
                const Divider(),
                const SizedBox(height: 8),
                LocalizedText('Role Assignment',
                    style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                _RoleAssignmentDropdown(
                  currentRole: tracker.assignedRole,
                  onChanged: onAssignRole,
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const LocalizedText('Mount-Flip (180°)'),
                  subtitle: const LocalizedText(
                      'Correct a tracker mounted with a 180-degree rotation.'),
                  value: mountFlip,
                  onChanged: onMountFlip,
                ),
                const SizedBox(height: 16),
                const Divider(),
                const SizedBox(height: 8),
                LocalizedText('Actions',
                    style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: actionButtons,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildInfoRow(String label, String value, {Color? color}) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 140,
            child: LocalizedText(
              label,
              style: TextStyle(
                color: Colors.grey[400],
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          Expanded(
            child: LocalizedText(
              value,
              style: TextStyle(color: color),
            ),
          ),
        ],
      ),
    );
  }

  String _formatPacketAge(Duration? age) {
    if (age == null) return 'Waiting for first IMU packet';
    if (age < const Duration(seconds: 1)) {
      return 'Live • ${age.inMilliseconds} ms ago';
    }
    if (age < const Duration(minutes: 1)) {
      return 'Delayed • ${age.inSeconds}s ago';
    }
    return 'No recent data • ${age.inMinutes}m ago';
  }

  String _vectorText(dynamic value, {bool quaternion = false}) {
    if (value is! Map) return 'Waiting for tracking data';
    final keys =
        quaternion ? const ['w', 'x', 'y', 'z'] : const ['x', 'y', 'z'];
    return keys.map((key) {
      final number = value[key];
      return number is num
          ? '${key.toUpperCase()} ${number.toStringAsFixed(2)}'
          : '$key —';
    }).join('  ');
  }

  Color _getStateColor(String state) {
    switch (state) {
      case 'Streaming':
        return Colors.green;
      case 'Discovered':
        return Colors.teal;
      case 'Discovering':
      case 'Connecting':
      case 'Initializing':
        return Colors.blue;
      case 'Reconnecting':
        return Colors.orange;
      case 'Error':
        return Colors.red;
      default:
        return Colors.grey;
    }
  }

  Color _getRssiColor(int rssi) {
    if (rssi == 0) return Colors.grey;
    if (rssi >= -50) return Colors.green;
    if (rssi >= -70) return Colors.yellow;
    return Colors.red;
  }
}

class _BatteryIndicator extends StatelessWidget {
  final BatteryInfo battery;

  const _BatteryIndicator({required this.battery});

  @override
  Widget build(BuildContext context) {
    final percentage = (battery.percentage * 100).toInt();
    Color color;
    if (percentage > 50) {
      color = Colors.green;
    } else if (percentage > 20) {
      color = Colors.orange;
    } else {
      color = Colors.red;
    }

    return Column(
      children: [
        Container(
          width: 40,
          height: 16,
          decoration: BoxDecoration(
            border: Border.all(color: Colors.grey),
            borderRadius: BorderRadius.circular(2),
          ),
          child: Stack(
            children: [
              FractionallySizedBox(
                widthFactor: battery.percentage.clamp(0.0, 1.0),
                child: Container(
                  decoration: BoxDecoration(
                    color: color,
                    borderRadius: BorderRadius.circular(1),
                  ),
                ),
              ),
              Positioned(
                right: 0,
                top: -2,
                child: Container(
                  width: 2,
                  height: 8,
                  color: Colors.grey,
                ),
              ),
            ],
          ),
        ),
        LocalizedText(
          '$percentage%',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(color: color),
        ),
      ],
    );
  }
}

class _RoleAssignmentDropdown extends StatelessWidget {
  final String? currentRole;
  final Function(String) onChanged;

  const _RoleAssignmentDropdown({
    required this.currentRole,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return DropdownButtonFormField<String>(
      initialValue: currentRole,
      decoration: const InputDecoration(
        label: LocalizedText('Assigned Role'),
        border: OutlineInputBorder(),
      ),
      items: [
        const DropdownMenuItem(value: null, child: LocalizedText('Unassigned')),
        ...TrackerRole.allRoles.map((role) => DropdownMenuItem(
              value: role,
              child: LocalizedText(role),
            )),
      ],
      onChanged: (value) {
        if (value != null) {
          onChanged(value);
        }
      },
    );
  }
}
