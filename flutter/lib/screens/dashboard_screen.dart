import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../widgets/localized_text.dart';
import '../state/app_state.dart';

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) context.read<AppState>().refresh();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        final trackers = appState.trackers;
        final connectedCount = trackers.where((t) => t.isConnected).length;
        final discoveredCount = trackers.length;
        final connected = trackers.where((t) => t.isConnected).toList();
        final roleCounts = <String, int>{};
        for (final tracker in connected) {
          final role = tracker.assignedRole;
          if (role != null) {
            roleCounts.update(role, (count) => count + 1, ifAbsent: () => 1);
          }
        }
        final duplicateRoleCount =
            roleCounts.values.where((count) => count > 1).length;
        final unassignedCount =
            connected.where((tracker) => tracker.assignedRole == null).length;
        final slimevrConnected = appState.slimevrConnected;
        final slime = appState.slimevrStatus;
        final adapterOk = appState.adapterOk;
        final reconnecting = appState.reconnectingCount;
        final hasAssignedRole = connected.any((t) => t.assignedRole != null);
        final hasSlimeTarget = slime?.enabled == true && slime?.ip != null;
        final bleValue = adapterOk == false
            ? 'No adapter'
            : appState.bleState == 'Scanning...'
                ? 'Scanning...'
                : reconnecting > 0
                    ? 'Reconnecting ($reconnecting)'
                    : 'Ready';
        final bleColor = adapterOk == false
            ? Colors.red
            : appState.bleState == 'Scanning...' || reconnecting > 0
                ? Colors.blue
                : Colors.green;
        final slimeValue = slime == null
            ? (slimevrConnected ? 'Connected' : 'Disconnected')
            : (!slime.enabled
                ? 'Disabled'
                : (slime.ip == null
                    ? 'No target'
                    : '${slime.ip}:${slime.port ?? ''}${slime.autoDiscovered ? ' (Auto)' : ''} • ${slime.packetsSent} pkt'));
        final statusCards = [
          _StatusCard(
            title: 'Connected Trackers',
            value: '$connectedCount / $discoveredCount',
            icon: Icons.bluetooth_connected,
            color: connectedCount > 0 ? Colors.green : Colors.red,
          ),
          _StatusCard(
            title: 'SlimeVR',
            value: slimeValue,
            icon: Icons.cloud,
            color: (slime?.enabled ?? slimevrConnected)
                ? (slime != null && slime.errors > 0 && slime.packetsSent == 0
                    ? Colors.orange
                    : Colors.green)
                : Colors.grey,
          ),
          _StatusCard(
            title: adapterOk == false ? 'BLE Adapter!' : 'BLE Status',
            value: bleValue,
            icon: Icons.bluetooth,
            color: bleColor,
          ),
        ];
        final lag = appState.eventLagged;
        final detailCards = [
          _StatusCard(
            title: 'Data Rate',
            value: '${appState.dataRate.toStringAsFixed(0)} pkt/s',
            icon: Icons.speed,
            color: appState.dataRate > 0 ? Colors.green : Colors.grey,
          ),
          _StatusCard(
            title: 'Errors',
            value: lag > 0
                ? '${appState.errorCount} (+Lag $lag)'
                : '${appState.errorCount}',
            icon: Icons.error_outline,
            color: (appState.errorCount > 0 || lag > 0)
                ? Colors.red
                : Colors.green,
          ),
        ];
        Widget cardRow(List<Widget> cards) {
          return LayoutBuilder(
            builder: (context, constraints) {
              if (constraints.maxWidth < 720) {
                return Column(
                  children: [
                    for (var i = 0; i < cards.length; i++) ...[
                      if (i > 0) const SizedBox(height: 12),
                      cards[i],
                    ],
                  ],
                );
              }
              return Row(
                children: [
                  for (var i = 0; i < cards.length; i++) ...[
                    if (i > 0) const SizedBox(width: 16),
                    Expanded(child: cards[i]),
                  ],
                ],
              );
            },
          );
        }

        final List<Widget> children = [
          // Status cards: horizontal on wide windows, wrapped on narrow
          // (the old fixed Row overflowed by ~117px at 800px width).
          cardRow(statusCards),
          const SizedBox(height: 16),
          cardRow(detailCards),
          const SizedBox(height: 12),
          Card(
            child: ExpansionTile(
              leading: const Icon(Icons.analytics_outlined),
              title: const LocalizedText('SlimeVR Output Diagnostics'),
              subtitle: LocalizedText(
                slime == null
                    ? 'Waiting for SlimeVR status'
                    : '${slime.ip ?? 'No target'}:${slime.port ?? 6969} • ${slime.packetsSent} packets • ${slime.errors} errors',
              ),
              children: [
                ListTile(
                  title: const LocalizedText('Output state'),
                  subtitle: LocalizedText(
                    slime == null
                        ? 'Status unavailable'
                        : slime.enabled
                            ? (slime.errors > 0 && slime.packetsSent == 0
                                ? 'No packets are being sent; check the server target and tracker roles.'
                                : 'SlimeVR output is enabled.')
                            : 'SlimeVR output is disabled in Settings.',
                  ),
                ),
                ListTile(
                  title: const LocalizedText('Streaming trackers'),
                  subtitle: LocalizedText(
                    '$connectedCount connected • $unassignedCount without a role • $duplicateRoleCount duplicate roles',
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          Card(
            child: ExpansionTile(
              leading: const Icon(Icons.checklist),
              title: const LocalizedText('Setup checklist'),
              subtitle: const LocalizedText(
                  'Follow these steps to send tracker data to SlimeVR.'),
              children: [
                _SetupCheckRow(
                  complete: adapterOk == true,
                  label: adapterOk == true
                      ? 'Bluetooth adapter ready'
                      : 'Check Bluetooth adapter',
                ),
                _SetupCheckRow(
                  complete: connectedCount > 0,
                  label: connectedCount > 0
                      ? 'Tracker connected'
                      : 'Connect at least one tracker',
                  onTap: () => Navigator.of(context).pushNamed('/trackers'),
                ),
                _SetupCheckRow(
                  complete: hasAssignedRole && duplicateRoleCount == 0,
                  label: duplicateRoleCount > 0
                      ? 'Resolve duplicate SlimeVR body roles'
                      : hasAssignedRole
                          ? 'Body role assigned'
                          : 'Assign a body role to a tracker',
                  onTap: () => Navigator.of(context).pushNamed('/trackers'),
                ),
                _SetupCheckRow(
                  complete: hasSlimeTarget,
                  label: hasSlimeTarget
                      ? 'SlimeVR server target configured'
                      : 'Configure the SlimeVR server target',
                  onTap: () => Navigator.of(context).pushNamed('/settings'),
                ),
                _SetupCheckRow(
                  complete: appState.isStreaming,
                  label: appState.isStreaming
                      ? 'Streaming is active'
                      : 'Start streaming from Quick Actions',
                ),
              ],
            ),
          ),
          if (appState.lastError != null) ...[
            const SizedBox(height: 16),
            Card(
              color: Theme.of(context).colorScheme.errorContainer,
              child: ListTile(
                leading: const Icon(Icons.warning),
                title: const LocalizedText('Last error'),
                subtitle: LocalizedText(appState.lastError!),
              ),
            ),
          ],
          const SizedBox(height: 24),
          // Quick Actions
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  LocalizedText(
                    'Quick Actions',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  const SizedBox(height: 16),
                  Wrap(
                    spacing: 12,
                    runSpacing: 12,
                    children: [
                      ElevatedButton.icon(
                        icon: appState.bleState == 'Scanning...'
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2),
                              )
                            : const Icon(Icons.search),
                        label: LocalizedText(appState.bleState == 'Scanning...'
                            ? 'Scanning...'
                            : 'Scan Devices'),
                        onPressed: appState.bleState == 'Scanning...'
                            ? null
                            : () => appState.scanDevices(),
                      ),
                      ElevatedButton.icon(
                        icon: const Icon(Icons.play_arrow),
                        label: const LocalizedText('Start Tracking'),
                        onPressed: appState.isStreaming
                            ? null
                            : () => appState.startStreaming(),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.green,
                        ),
                      ),
                      ElevatedButton.icon(
                        icon: const Icon(Icons.stop),
                        label: const LocalizedText('Stop Tracking'),
                        onPressed: !appState.isStreaming
                            ? null
                            : () => appState.stopStreaming(),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.red,
                        ),
                      ),
                      OutlinedButton.icon(
                        icon: const Icon(Icons.cloud_sync),
                        label: const LocalizedText('SlimeVR Reconnect'),
                        onPressed: () => appState.reconnectSlimevr(),
                      ),
                      OutlinedButton.icon(
                        icon: const Icon(Icons.handshake),
                        label: const LocalizedText('Handshake senden'),
                        onPressed: appState.slimevrStatus?.enabled == true ||
                                appState.slimevrConnected
                            ? () => appState.resendHandshake()
                            : null,
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 24),
          // Tracker Overview
          LocalizedText(
            'Trackers Overview',
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(height: 16),
        ];

        if (trackers.isEmpty) {
          children.add(
            Card(
              child: Padding(
                padding: const EdgeInsets.all(32),
                child: Center(
                  child: Column(
                    children: [
                      const Icon(Icons.devices, size: 64, color: Colors.grey),
                      const SizedBox(height: 16),
                      LocalizedText(
                        'No trackers found',
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                      const SizedBox(height: 8),
                      LocalizedText(
                        'Click "Scan Devices" to discover Mocopi trackers',
                        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                              color: Colors.grey,
                            ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        } else {
          for (final tracker in trackers) {
            children.add(_TrackerSummaryCard(tracker: tracker));
            children.add(const SizedBox(height: 8));
          }
        }

        return SingleChildScrollView(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: children,
          ),
        );
      },
    );
  }
}

class _SetupCheckRow extends StatelessWidget {
  final bool complete;
  final String label;
  final VoidCallback? onTap;

  const _SetupCheckRow({
    required this.complete,
    required this.label,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      dense: true,
      leading: Icon(
        complete ? Icons.check_circle : Icons.radio_button_unchecked,
        color: complete ? Colors.green : Colors.grey,
      ),
      title: LocalizedText(label),
      trailing: onTap == null ? null : const Icon(Icons.chevron_right),
      onTap: onTap,
    );
  }
}

class _TrackerSummaryCard extends StatelessWidget {
  final TrackerStatus tracker;

  const _TrackerSummaryCard({required this.tracker});

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: _getStateColor(tracker.state),
          child: Icon(
            tracker.isConnected
                ? Icons.check_circle
                : Icons.radio_button_unchecked,
            color: Colors.white,
          ),
        ),
        title: LocalizedText(tracker.name),
        subtitle: LocalizedText(
            '${tracker.bluetoothAddress} • ${tracker.assignedRole ?? 'Unassigned'}'),
        trailing: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            if (tracker.battery != null)
              LocalizedText(
                '${(tracker.battery!.percentage * 100).toInt()}%',
                style: Theme.of(context).textTheme.bodySmall,
              ),
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
          ],
        ),
      ),
    );
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

class _StatusCard extends StatelessWidget {
  final String title;
  final String value;
  final IconData icon;
  final Color color;

  const _StatusCard({
    required this.title,
    required this.value,
    required this.icon,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Icon(icon, color: color, size: 28),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  LocalizedText(title,
                      style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 4),
                  LocalizedText(value, overflow: TextOverflow.ellipsis),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
