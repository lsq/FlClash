import 'dart:convert';
import 'dart:typed_data';

/// Field names whose scalar values must always be treated as strings by any
/// YAML parser (Dart's or Go/mihomo's yaml.v3), because plain (unquoted)
/// numeric-looking scalars get silently reinterpreted as integers/floats,
/// which strips things like leading zeros (e.g. `short-id: 08` -> `8`).
const _kStringSemanticFields = <String>{
  'short-id',
  'uuid',
  'password',
  'alterId', // keep as-is elsewhere if it's genuinely numeric upstream
};

/// Matches a mapping entry `<indent><field>:<spaces><value><trailing>` where
/// `<value>` is NOT already quoted, NOT a flow collection (`[...]`/`{...}`),
/// and is not empty/a comment. Only rewrites the value in-place with double
/// quotes, preserving indentation and inline comments/whitespace.
final RegExp _kFieldValuePattern = RegExp(
  r'^([ \t]*(?:-[ \t]+)?(?:' +
      _kStringSemanticFields.map(RegExp.escape).join('|') +
      r')[ \t]*:)[ \t]*'
          r'([^\s"'
          "'"
          r'#\[\{][^#\r\n]*?)[ \t]*(#.*)?$',
  multiLine: true,
);

/// Rewrites known string-semantic fields in a raw YAML document so that
/// unquoted scalars (e.g. `short-id: 08`, `uuid: 0123`) are wrapped in double
/// quotes before any YAML parser (Dart's `loadYaml` or Go core's
/// `executor.ParseWithBytes`) has a chance to run implicit-typing resolution
/// on them and silently mangle the literal text (dropping leading zeros,
/// turning `0123` into octal-looking garbage, truncating via float
/// conversion, etc).
///
/// This is a best-effort textual patch: it does not do full YAML parsing, so
/// it deliberately skips values that are already quoted, are flow
/// collections, or are empty, to avoid double-quoting or corrupting valid
/// YAML.
String quoteKnownStringFields(String rawYaml) {
  return rawYaml.replaceAllMapped(_kFieldValuePattern, (match) {
    final prefix = match.group(1)!; // "  short-id:" (with any leading "- ")
    final rawValue = match.group(2)!.trim();
    final comment = match.group(3);

    // Escape any embedded double quotes/backslashes so we don't produce
    // invalid YAML if the original (unquoted) value happened to contain them.
    final escaped = rawValue.replaceAll('\\', '\\\\').replaceAll('"', '\\"');

    final quotedValue = '"$escaped"';
    return comment == null
        ? '$prefix $quotedValue'
        : '$prefix $quotedValue $comment';
  });
}

/// Byte-level convenience wrapper for use where content arrives as
/// [Uint8List] (subscription download body / picked file bytes), before it
/// is written to disk or handed to any parser/validator.
Uint8List quoteKnownStringFieldsInBytes(Uint8List bytes) {
  final decoded = utf8.decode(bytes, allowMalformed: true);
  final patched = quoteKnownStringFields(decoded);
  return Uint8List.fromList(utf8.encode(patched));
}
