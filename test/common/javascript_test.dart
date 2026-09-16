import 'dart:convert';

import 'package:fl_clash/common/exception.dart';
import 'package:fl_clash/common/javascript.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  late List<({String script, String config, String? proxy})> calls;
  final engine = scriptEvaluator;
  final container = ProviderContainer();

  setUp(() {
    calls = [];
  });

  tearDown(() {
    scriptEvaluator = engine;
  });

  void answerWith(String result) {
    scriptEvaluator =
        ({
          required String script,
          required String config,
          String? proxy,
        }) async {
          calls.add((script: script, config: config, proxy: proxy));
          return result;
        };
  }

  test(
    'passes the profile as JSON and returns what the script produced',
    () async {
      answerWith(json.encode({'mode': 'global'}));

      final result = await handleEvaluate('main', {
        'mode': 'rule',
      }, read: container.read);

      expect(result, {'mode': 'global'});
      expect(json.decode(calls.single.config)['mode'], 'rule');
      expect(calls.single.script, 'main');
    },
  );

  test('gives the script an empty proxy-providers map to work with', () async {
    answerWith(json.encode({'mode': 'rule'}));

    await handleEvaluate('main', {'mode': 'rule'}, read: container.read);

    expect(json.decode(calls.single.config)['proxy-providers'], isEmpty);
  });

  test('reports what the engine threw', () async {
    scriptEvaluator =
        ({
          required String script,
          required String config,
          String? proxy,
        }) async {
          throw Exception('ReferenceError: proxies is not defined');
        };

    await expectLater(
      handleEvaluate('main', {}, read: container.read),
      throwsA(
        isA<MessageException>().having(
          (error) => error.message,
          'message',
          contains('ReferenceError'),
        ),
      ),
    );
  });

  test('rejects a result that is not a configuration object', () async {
    answerWith(json.encode([1, 2, 3]));

    await expectLater(
      handleEvaluate('main', {}, read: container.read),
      throwsA(isA<MessageException>()),
    );
  });
}
