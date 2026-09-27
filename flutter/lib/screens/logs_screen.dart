import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../widgets/localized_text.dart';
import '../state/app_state.dart';

/// GUI log view (Spec §17): Rust ring buffer (1 Hz poll) plus captured
/// Dart/Flutter errors, newest last, with level filter + text search +
/// copy/clear/poll actions and a one-file diagnostics export
/// (`%APPDATA%\Mocoslime\diagnostics-<ts>.json`) for bug reports.
class LogsScreen extends StatefulWidget {
  const LogsScreen({super.key});

  @override
  State<LogsScreen> createState() => _LogsScreenState();
}

class _LogsScreenState extends State<LogsScreen> {
  String _minLevel = 'All';
  String _query = '';
  final TextEditingController _searchController = TextEditingController();
  bool _exporting = false;

  static const _levels = ['All', 'ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'];

  static int _severity(String level) {
    switch (level.toUpperCase()) {
      case 'ERROR':
        return 4;
      case 'WARN':
        return 3;
      case 'INFO':
        return 2;
      case 'DEBUG':
        return 1;
      case 'TRACE':
        return 0;
      default:
        return 2;
    }
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Color _levelColor(String level, BuildContext context) {
    switch (level.toUpperCase()) {
      case 'ERROR':
        return Colors.red;
      case 'WARN':
        return Colors.orange;
      case 'INFO':
        return Theme.of(context).colorScheme.primary;
      case 'DEBUG':
        return Colors.grey;
      case 'TRACE':
        return Colors.grey.shade600;
      default:
        return Colors.grey;
    }
  }

  List<LogEntry> _filtered(List<LogEntry> logs) {
    final q = _query.trim().toLowerCase();
    return logs.where((e) {
      if (_minLevel != 'All' && _severity(e.level) < _severity(_minLevel)) {
        return false;
      }
      if (q.isEmpty) return true;
      return e.message.toLowerCase().contains(q) ||
          e.target.toLowerCase().contains(q);
    }).toList();
  }

  Future<void> _exportDiagnostics(
      BuildContext context, AppState appState) async {
    var includePrivateInfo = false;
    final include = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const LocalizedText('Export support bundle'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const LocalizedText(
                'IP addresses, device identifiers, and local paths are removed by default.',
              ),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                value: includePrivateInfo,
                title: const LocalizedText(
                    'Include IP addresses and device identifiers'),
                subtitle: const LocalizedText(
                    'Enable this only when the support recipient needs them.'),
                onChanged: (value) =>
                    setDialogState(() => includePrivateInfo = value ?? false),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(null),
              child: const LocalizedText('Cancel'),
            ),
            FilledButton(
              onPressed: () =>
                  Navigator.of(dialogContext).pop(includePrivateInfo),
              child: const LocalizedText('Export'),
            ),
          ],
        ),
      ),
    );
    if (include == null || !context.mounted) return;
    setState(() => _exporting = true);
    try {
      final path = await appState.exportDiagnostics(
        redactPrivateInfo: !include,
      );
      if (!context.mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: LocalizedText(path == null
              ? (appState.lastError ?? 'Diagnostics export failed')
              : 'Diagnostics saved: $path'),
        ),
      );
    } finally {
      if (mounted) setState(() => _exporting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        final logs = appState.logs;
        final visible = _filtered(logs);
        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: Row(
                children: [
                  Expanded(
                    child: LocalizedText(
                      'Logs (${visible.length}/${logs.length})',
                      style: Theme.of(context).textTheme.headlineSmall,
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.copy),
                    tooltip: localizeUiString(
                        Localizations.localeOf(context).countryCode == 'CN'
                            ? 'zh-CN'
                            : Localizations.localeOf(context).languageCode,
                        'Copy visible logs'),
                    onPressed: visible.isEmpty
                        ? null
                        : () {
                            final text = visible
                                .map((e) =>
                                    '${e.timestamp} [${e.level}] ${e.target}: ${e.message}')
                                .join('\n');
                            Clipboard.setData(ClipboardData(text: text));
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(
                                  content: LocalizedText('Logs copied')),
                            );
                          },
                  ),
                  IconButton(
                    icon: _exporting
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.bug_report_outlined),
                    tooltip: localizeUiString(
                        Localizations.localeOf(context).countryCode == 'CN'
                            ? 'zh-CN'
                            : Localizations.localeOf(context).languageCode,
                        'Export diagnostics file (support bundle)'),
                    onPressed: _exporting
                        ? null
                        : () => _exportDiagnostics(context, appState),
                  ),
                  IconButton(
                    icon: const Icon(Icons.delete_outline),
                    tooltip: localizeUiString(
                        Localizations.localeOf(context).countryCode == 'CN'
                            ? 'zh-CN'
                            : Localizations.localeOf(context).languageCode,
                        'Clear'),
                    onPressed: logs.isEmpty ? null : () => appState.clearLogs(),
                  ),
                  IconButton(
                    icon: const Icon(Icons.refresh),
                    tooltip: localizeUiString(
                        Localizations.localeOf(context).countryCode == 'CN'
                            ? 'zh-CN'
                            : Localizations.localeOf(context).languageCode,
                        'Poll now'),
                    onPressed: () => appState.drainLogs(),
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Row(
                children: [
                  LocalizedTooltip(
                    message: 'Minimum level',
                    child: DropdownButton<String>(
                      value: _minLevel,
                      items: _levels
                          .map((l) => DropdownMenuItem(
                              value: l, child: LocalizedText(l)))
                          .toList(),
                      onChanged: (v) => setState(() => _minLevel = v ?? 'All'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: TextField(
                      controller: _searchController,
                      decoration: InputDecoration(
                        hintText: localizeUiString(
                            Localizations.localeOf(context).countryCode == 'CN'
                                ? 'zh-CN'
                                : Localizations.localeOf(context).languageCode,
                            'Search message or target…'),
                        prefixIcon: const Icon(Icons.search),
                        border: const OutlineInputBorder(),
                        isDense: true,
                      ),
                      onChanged: (v) => setState(() => _query = v),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            if (logs.isEmpty)
              Expanded(
                child: Center(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const Icon(Icons.article_outlined,
                          size: 80, color: Colors.grey),
                      const SizedBox(height: 24),
                      LocalizedText(
                        'No log entries yet',
                        style: Theme.of(context).textTheme.headlineMedium,
                      ),
                      const SizedBox(height: 12),
                      LocalizedText(
                        'Rust logs appear here once the core is running.\n'
                        'Full history: %APPDATA%\\Mocoslime\\logs\\mocoslime.log',
                        textAlign: TextAlign.center,
                        style: Theme.of(context)
                            .textTheme
                            .bodyLarge
                            ?.copyWith(color: Colors.grey),
                      ),
                    ],
                  ),
                ),
              )
            else if (visible.isEmpty)
              Expanded(
                child: Center(
                  child: LocalizedText(
                    'No entries match filter/search',
                    style: Theme.of(context)
                        .textTheme
                        .bodyLarge
                        ?.copyWith(color: Colors.grey),
                  ),
                ),
              )
            else
              Expanded(
                child: ListView.builder(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  itemCount: visible.length,
                  itemBuilder: (context, index) {
                    final entry = visible[index];
                    return Card(
                      margin: const EdgeInsets.only(bottom: 8),
                      child: ListTile(
                        dense: true,
                        leading: Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 8, vertical: 4),
                          decoration: BoxDecoration(
                            color: _levelColor(entry.level, context)
                                .withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: LocalizedText(
                            entry.level,
                            style: TextStyle(
                              color: _levelColor(entry.level, context),
                              fontWeight: FontWeight.bold,
                              fontSize: 12,
                            ),
                          ),
                        ),
                        title: LocalizedText(
                          entry.message,
                          style: const TextStyle(
                              fontFamily: 'monospace', fontSize: 13),
                        ),
                        subtitle: LocalizedText(
                          '${entry.timestamp} • ${entry.target}',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                    );
                  },
                ),
              ),
          ],
        );
      },
    );
  }
}
