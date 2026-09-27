import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/app_state.dart';
import '../widgets/localized_text.dart';
import '../services/native_bridge.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        return SingleChildScrollView(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (!appState.isNativeReady) ...[
                Card(
                  color: Theme.of(context).colorScheme.errorContainer,
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            Icon(Icons.error_outline,
                                color: Theme.of(context)
                                    .colorScheme
                                    .onErrorContainer),
                            const SizedBox(width: 12),
                            Expanded(
                              child: LocalizedText(
                                'Native Bridge Unavailable',
                                style: Theme.of(context)
                                    .textTheme
                                    .titleMedium
                                    ?.copyWith(
                                      color: Theme.of(context)
                                          .colorScheme
                                          .onErrorContainer,
                                    ),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 8),
                        LocalizedText(
                          appState.lastError ??
                              'Native bridge (moslime_ffi.dll) could not be loaded.',
                          style:
                              Theme.of(context).textTheme.bodyMedium?.copyWith(
                                    color: Theme.of(context)
                                        .colorScheme
                                        .onErrorContainer,
                                  ),
                        ),
                        const SizedBox(height: 12),
                        ElevatedButton.icon(
                          icon: const Icon(Icons.refresh),
                          label: const LocalizedText('Retry Initialization'),
                          onPressed: () async {
                            await NativeBridge.initialize();
                            if (NativeBridge.isNativeAvailable) {
                              await appState.initialize();
                            }
                          },
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
              ],
              _buildSection(context, 'General', [
                _buildSwitchTile(
                  'Start with Windows',
                  appState.settings.startWithWindows,
                  (value) => appState.updateSetting('startWithWindows', value),
                ),
                _buildSwitchTile(
                  'Start Minimized',
                  appState.settings.startMinimized,
                  (value) => appState.updateSetting('startMinimized', value),
                ),
                _buildDropdownTile(
                  'Language',
                  appState.settings.language,
                  ['en', 'ja', 'zh-CN', 'de'],
                  (value) => appState.updateSetting('language', value!),
                ),
                _buildDropdownTile(
                  'Log Level',
                  appState.settings.logLevel,
                  ['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'],
                  (value) => appState.updateSetting('logLevel', value!),
                ),
              ]),
              _buildSection(context, 'Bluetooth', [
                _buildSwitchTile(
                  'Auto Scan',
                  appState.settings.bluetoothAutoScan,
                  (value) => appState.updateSetting('bluetoothAutoScan', value),
                ),
                _buildSwitchTile(
                  'Auto Connect',
                  appState.settings.bluetoothAutoConnect,
                  (value) =>
                      appState.updateSetting('bluetoothAutoConnect', value),
                ),
                _buildSliderTile(
                  'Reconnect Delay (ms)',
                  appState.settings.bluetoothReconnectDelayMs.toDouble(),
                  1000,
                  30000,
                  onChanged: (value) => appState.updateSetting(
                      'bluetoothReconnectDelayMs', value.round()),
                ),
                _buildSliderTile(
                  'Connection Timeout (ms)',
                  appState.settings.bluetoothConnectionTimeoutMs.toDouble(),
                  5000,
                  60000,
                  onChanged: (value) => appState.updateSetting(
                      'bluetoothConnectionTimeoutMs', value.round()),
                ),
                _buildSliderTile(
                  'Max Trackers',
                  appState.settings.bluetoothMaxTrackers.toDouble(),
                  1,
                  12,
                  onChanged: (value) => appState.updateSetting(
                      'bluetoothMaxTrackers', value.round()),
                  divisions: 11,
                ),
                _buildSliderTile(
                  'Min. Signal (dBm)',
                  appState.settings.bluetoothRssiMinDbm.toDouble(),
                  -100,
                  -40,
                  onChanged: (value) => appState.updateSetting(
                      'bluetoothRssiMinDbm', value.round()),
                  divisions: 12,
                ),
              ]),
              _buildSection(context, 'SlimeVR', [
                _buildSwitchTile(
                  'Enabled',
                  appState.settings.slimevrEnabled,
                  (value) => appState.updateSetting('slimevrEnabled', value),
                ),
                _buildTextFieldTile(
                  'Server IP',
                  appState.settings.slimevrServerIp,
                  (value) => appState.updateSetting('slimevrServerIp', value),
                ),
                _buildNumberFieldTile(
                  'Server Port',
                  appState.settings.slimevrServerPort,
                  (value) => appState.updateSetting('slimevrServerPort', value),
                ),
                _buildSliderTile(
                  'Packet Rate (Hz)',
                  appState.settings.slimevrPacketRate.toDouble(),
                  30,
                  200,
                  onChanged: (value) => appState.updateSetting(
                      'slimevrPacketRate', value.round()),
                ),
              ]),
              _buildSection(context, 'Tracking', [
                _buildDropdownTile(
                  'Coordinate System',
                  appState.settings.trackingCoordinateSystem,
                  ['Mocopi', 'SlimeVR', 'OpenVR', 'Unity'],
                  (value) => appState.updateSetting(
                      'trackingCoordinateSystem', value!),
                ),
                _buildSwitchTile(
                  'Enable Filtering',
                  appState.settings.trackingEnableFiltering,
                  (value) =>
                      appState.updateSetting('trackingEnableFiltering', value),
                ),
                _buildSliderTile(
                  'Quaternion Slerp Factor',
                  appState.settings.trackingFilterSlerpFactor,
                  0.0,
                  1.0,
                  onChanged: (value) => appState.updateSetting(
                      'trackingFilterSlerpFactor', value),
                ),
                _buildSliderTile(
                  'Accel Low-pass Alpha',
                  appState.settings.trackingFilterAccelAlpha,
                  0.0,
                  1.0,
                  onChanged: (value) =>
                      appState.updateSetting('trackingFilterAccelAlpha', value),
                ),
              ]),
              const SizedBox(height: 24),
              Center(
                child: ElevatedButton.icon(
                  icon: const Icon(Icons.save),
                  label: const LocalizedText('Save Settings'),
                  onPressed: () => appState.saveSettings(),
                  style: ElevatedButton.styleFrom(
                    minimumSize: const Size(200, 48),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildSection(
      BuildContext context, String title, List<Widget> children) {
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: ExpansionTile(
        title:
            LocalizedText(title, style: Theme.of(context).textTheme.titleLarge),
        initiallyExpanded: true,
        children: children
            .map((c) => Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
                  child: c,
                ))
            .toList(),
      ),
    );
  }

  Widget _buildSwitchTile(
      String title, bool value, ValueChanged<bool> onChanged) {
    return SwitchListTile(
      title: LocalizedText(title),
      value: value,
      onChanged: onChanged,
    );
  }

  Widget _buildDropdownTile(String title, String value, List<String> options,
      ValueChanged<String?> onChanged) {
    return ListTile(
      title: LocalizedText(title),
      trailing: DropdownButton<String>(
        value: value,
        items: options
            .map((e) => DropdownMenuItem(
                  value: e,
                  child: LocalizedText(title == 'Language'
                      ? {
                            'en': 'English',
                            'ja': 'Japanese',
                            'zh-CN': 'Chinese (Simplified)',
                            'de': 'German',
                          }[e] ??
                          e
                      : e),
                ))
            .toList(),
        onChanged: onChanged,
        underline: const SizedBox(),
      ),
    );
  }

  Widget _buildSliderTile(
    String title,
    double value,
    double min,
    double max, {
    required ValueChanged<double> onChanged,
    int? divisions,
  }) {
    return ListTile(
      title: LocalizedText(
          '$title: ${divisions != null ? value.round() : value.toStringAsFixed(2)}'),
      subtitle: Slider(
        value: value,
        min: min,
        max: max,
        divisions: divisions,
        onChanged: onChanged,
      ),
    );
  }

  Widget _buildTextFieldTile(
      String title, String value, ValueChanged<String> onChanged) {
    return ListTile(
      title: LocalizedText(title),
      trailing: SizedBox(
        width: 200,
        child: _CommitTextField(
          initialValue: value,
          keyboardType: TextInputType.text,
          onCommit: onChanged,
        ),
      ),
    );
  }

  Widget _buildNumberFieldTile(
      String title, int value, ValueChanged<int> onChanged) {
    return ListTile(
      title: LocalizedText(title),
      trailing: SizedBox(
        width: 100,
        child: _CommitTextField(
          initialValue: value.toString(),
          keyboardType: TextInputType.number,
          onCommit: (v) => onChanged(int.tryParse(v) ?? value),
        ),
      ),
    );
  }
}

/// Text field that owns its controller (no rebuild cursor jumps) and only
/// commits on submit, so every keystroke doesn't trigger an FFI roundtrip.
class _CommitTextField extends StatefulWidget {
  final String initialValue;
  final TextInputType keyboardType;
  final ValueChanged<String> onCommit;

  const _CommitTextField({
    required this.initialValue,
    required this.keyboardType,
    required this.onCommit,
  });

  @override
  State<_CommitTextField> createState() => _CommitTextFieldState();
}

class _CommitTextFieldState extends State<_CommitTextField> {
  late final TextEditingController _controller;
  late String _committed;

  @override
  void initState() {
    super.initState();
    _committed = widget.initialValue;
    _controller = TextEditingController(text: widget.initialValue);
  }

  @override
  void didUpdateWidget(covariant _CommitTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Adopt external changes (e.g. after reload), never clobber typing.
    if (widget.initialValue != _committed &&
        widget.initialValue != _controller.text) {
      _committed = widget.initialValue;
      _controller.text = widget.initialValue;
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _commit() {
    if (_controller.text != _committed) {
      _committed = _controller.text;
      widget.onCommit(_committed);
    }
    FocusScope.of(context).unfocus();
  }

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: _controller,
      decoration: const InputDecoration(
        border: OutlineInputBorder(),
        isDense: true,
      ),
      keyboardType: widget.keyboardType,
      textInputAction: TextInputAction.done,
      onSubmitted: (_) => _commit(),
      onTapOutside: (_) => _commit(),
    );
  }
}
