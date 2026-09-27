import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/app_state.dart';
import '../widgets/localized_text.dart';

class CalibrationScreen extends StatelessWidget {
  const CalibrationScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        final status = appState.slimevrStatus;
        final target = status?.ip == null
            ? 'No SlimeVR server connected'
            : '${status!.ip}:${status.port ?? 6969}';
        final streaming =
            appState.trackers.where((tracker) => tracker.isConnected).length;

        return ListView(
          padding: const EdgeInsets.all(24),
          children: [
            LocalizedText('SlimeVR Calibration',
                style: Theme.of(context).textTheme.headlineMedium),
            const SizedBox(height: 12),
            const LocalizedText(
              'SlimeVR Server manages body and tracker calibration. Mocoslime forwards tracker orientation after hardware mounting correction, coordinate conversion, and sensor filtering. It does not apply additional pose offsets.',
            ),
            const SizedBox(height: 20),
            Card(
              child: ListTile(
                leading: const Icon(Icons.cloud),
                title: const LocalizedText('SlimeVR Server'),
                subtitle:
                    LocalizedText('$target\n$streaming connected trackers'),
                isThreeLine: true,
              ),
            ),
            const SizedBox(height: 16),
            const Card(
              child: Padding(
                padding: EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    LocalizedText('Calibration steps',
                        style: TextStyle(fontWeight: FontWeight.bold)),
                    SizedBox(height: 8),
                    LocalizedText(
                        '1. Connect trackers here and register them with SlimeVR.'),
                    LocalizedText(
                        '2. Assign trackers to body roles in SlimeVR Server.'),
                    LocalizedText(
                        '3. Follow the reference pose shown by SlimeVR and calibrate there.'),
                    LocalizedText(
                        '4. Perform drift resets and re-alignment in SlimeVR Server.'),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                ElevatedButton.icon(
                  onPressed: () => appState.reconnectSlimevr(),
                  icon: const Icon(Icons.cloud_sync),
                  label: const LocalizedText('Connect SlimeVR'),
                ),
                OutlinedButton.icon(
                  onPressed:
                      appState.slimevrConnected || status?.enabled == true
                          ? () => appState.resendHandshake()
                          : null,
                  icon: const Icon(Icons.handshake),
                  label: const LocalizedText('Re-register trackers'),
                ),
              ],
            ),
          ],
        );
      },
    );
  }
}
