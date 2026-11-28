#!/usr/bin/env bash
# Instant checks that need nothing but Node (no builds, no installs): run before every push.
# All tests run in GitHub Actions; nothing is built or tested on this machine.
set -euo pipefail
cd "$(dirname "$0")/.."
echo "== syntax of every JavaScript file"
for f in web/js/*.js tests-js/*.mjs e2e/*.mjs; do node --check "$f"; done
echo "preflight ok"
