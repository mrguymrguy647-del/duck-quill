#!/usr/bin/env bash
# Tries Duck Quill 0.5.0's real quill on pictures saved by this version
# (crates/engine/tests/fixtures/*-0.6.0.quill). Run from the repository's
# root with the path of an old quill:
#
#   .github/compat/old-reader.sh path/to/0.5.0/quill
#
# - A picture with no frames is saved exactly as 0.5.0 saved pictures:
#   0.5.0 opens it and paints exactly the same pixels.
# - An animated picture (file version 2): 0.5.0 says it was saved by a
#   newer version and stops. It never opens part of it (where Save would
#   then lose the other frames) and never crashes.
set -euo pipefail
old="$1"
fixtures=crates/engine/tests/fixtures
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
echo "Old quill: $("$old" --version)"

echo "1. A plain picture from this version: 0.5.0 opens it, same pixels."
"$old" info "$fixtures/plain-0.6.0.quill"
"$old" render .github/compat/open-plain.json --out "$out" > /dev/null
cmp "$out/open-plain.png" "$fixtures/plain-0.6.0.png"
echo "   same pixels, byte for byte"

echo "2. An animated picture: 0.5.0 refuses it with a clear message."
set +e
"$old" info "$fixtures/walk-0.6.0.quill" > "$out/stdout.txt" 2> "$out/stderr.txt"
code=$?
set -e
cat "$out/stderr.txt"
if [[ $code -ne 1 ]]; then
  echo "expected exit code 1 (a clear error), got $code"
  exit 1
fi
grep -q "saved by a newer version of Duck Quill (file version 2)" "$out/stderr.txt"
grep -q "Please update Duck Quill" "$out/stderr.txt"
if [[ -s "$out/stdout.txt" ]]; then
  echo "it printed something as if it had opened the picture:"
  cat "$out/stdout.txt"
  exit 1
fi
echo "   refused, with the update message"
echo "OK: 0.5.0 reads this version's pictures as it should."
