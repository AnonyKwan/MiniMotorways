#!/usr/bin/env bash
# Publishes the current branch to main (and so to GitHub Pages) ONLY if CI has finished green for exactly this commit.
# CI writes `status.log` on the ci-logs branch; this script reads it.
set -euo pipefail
cd "$(dirname "$0")/.."
sha="$(git rev-parse HEAD)"
git fetch -q origin ci-logs
status="$(git show origin/ci-logs:status.log 2>/dev/null | head -3 || true)"
if ! echo "$status" | grep -q "$sha" || ! echo "$status" | grep -q "PASS"; then
  echo "Not shipping: CI has not reported a green run for $sha yet." >&2
  echo "status.log says:" >&2
  echo "$status" >&2
  exit 1
fi
git push origin HEAD:main
echo "Shipped $sha to main."
