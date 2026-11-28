#!/usr/bin/env bash
# Instant checks that need nothing but Node (no builds, no installs): run before every push.
# All tests run in GitHub Actions; nothing is built or tested on this machine.
set -euo pipefail
cd "$(dirname "$0")/.."
echo "== syntax of every JavaScript file"
for f in web/js/*.js tests-js/*.mjs e2e/*.mjs; do node --check "$f"; done
echo "== data tables are in sync with the generators"
if command -v python3 >/dev/null 2>&1 || command -v python >/dev/null 2>&1; then
  python_bin="$(command -v python3 || command -v python)"
  "$python_bin" -c "import ast,sys; [ast.parse(open(p,encoding='utf-8').read()) for p in ['tools/gen_cities.py','tools/gen_achievements.py']]"
fi
echo "preflight ok"
