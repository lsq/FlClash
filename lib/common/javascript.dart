import 'dart:convert';

import 'package:fl_clash/common/exception.dart';
import 'package:fl_clash/common/provider_reader.dart';
import 'package:fl_clash/providers/config.dart';
import 'package:fl_clash/providers/state.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rust_api/rust_api.dart';

typedef ScriptEvaluator =
    Future<String> Function({
      required String script,
      required String config,
      String? proxy,
    });

@visibleForTesting
ScriptEvaluator scriptEvaluator = evaluateScript;

Future<Map<String, dynamic>> handleEvaluate(
  String scriptContent,
  Map<String, dynamic> config, {
  required ProviderReader read,
}) async {
  if (config['proxy-providers'] == null) {
    config['proxy-providers'] = {};
  }
  final String result;
  try {
    result = await scriptEvaluator(
      script: scriptContent,
      config: json.encode(config),
      proxy: _resolveScriptProxy(read),
    );
  } catch (e) {
    throw MessageException(e.toString());
  }
  final decoded = json.decode(result);
  if (decoded is! Map<String, dynamic>) {
    throw const MessageException(
      'script did not return a configuration object',
    );
  }
  return decoded;
}

/// Mirrors `FlClashHttpOverrides.findProxyForReader`: fetch() inside a
/// profile script must go through mihomo's own mixed port whenever the core
/// (and possibly TUN) is active, otherwise a raw socket opened by the script
/// engine gets captured and aborted by the device's own tunnel routes.
String? _resolveScriptProxy(ProviderReader read) {
  final isStart = read(isStartProvider);
  final suspend = read(suspendProvider);
  if (!isStart || suspend) return null;
  final mixedPort = read(
    patchClashConfigProvider.select((state) => state.mixedPort),
  );
  return 'http://127.0.0.1:$mixedPort';
}
