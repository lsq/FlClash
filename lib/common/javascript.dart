import 'dart:convert';
import 'dart:ffi' as ffi;

import 'package:flutter/services.dart';
import 'package:flutter_js/flutter_js.dart';
// import 'package:flutter_js/extensions/xhr.dart';
import 'package:flutter_js/extensions/fetch.dart';
// import 'package:flutter_js/extensions/handle_promises.dart';

Future<Map<String, dynamic>> handleEvaluate(
  String scriptContent,
  Map<String, dynamic> config,
) async {
  if (config['proxy-providers'] == null) {
    config['proxy-providers'] = {};
  }
  final configJs = json.encode(config);
  final runtime = getJavascriptRuntime();
  final jsYamlSource = await rootBundle.loadString('assets/js/js-yaml.min.js');
  await runtime.enableFetch();
  runtime.evaluate(jsYamlSource);
  final jsEvalRes = await runtime.evaluateAsync('''
      $scriptContent
      main($configJs)
    ''');
  runtime.executePendingJob();
  final res = await runtime.handlePromise(jsEvalRes);
  if (res.isError) {
    throw res.stringResult;
  }
  var raw = res.rawResult;
  if (raw is Future) {
    raw = await raw;
  }

  // 2. Safe casting logic
  Map<String, dynamic> value;
 
  if (raw is Map) {
    value = Map<String, dynamic>.from(raw);
  } else if (raw is ffi.Pointer) {
    value = runtime.convertValue<Map<String, dynamic>>(res) ?? config;
  } else {
    // If it's a JSON string (common in some flutter_js versions)
    try {
      value = Map<String, dynamic>.from(json.decode(res.stringResult));
    } catch (_) {
      value = config;
    }
  }

  return value;

  // final value = switch (res.rawResult is ffi.Pointer) {
  //   true => runtime.convertValue<Map<String, dynamic>>(res),
  //   false => Map<String, dynamic>.from(res.rawResult),
  // };
  // return value ?? config;
}
