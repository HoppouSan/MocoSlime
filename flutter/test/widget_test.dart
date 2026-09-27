import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';

import 'package:mocoslime/main.dart';
import 'package:mocoslime/services/native_bridge.dart';
import 'package:mocoslime/state/app_state.dart';

void main() {
  testWidgets('App starts smoke test', (WidgetTester tester) async {
    final bridge = NativeBridge();
    final appState = AppState(bridge);
    await tester.pumpWidget(
      MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: appState),
        ],
        child: const MocoslimeApp(),
      ),
    );
    await tester.pump();

    expect(find.text('Dashboard'), findsWidgets);
    expect(find.text('Trackers'), findsWidgets);
  });
}
