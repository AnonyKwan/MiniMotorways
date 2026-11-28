#!/usr/bin/env bash
# Copies files (e.g. screenshots) onto the `ci-logs` branch under shots/ so they can be looked at.
set -u
dir="$(realpath "$1")"
[ -d "$dir" ] || exit 0
work="$(mktemp -d)"
url="https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
git clone --quiet --depth 1 --branch ci-logs "$url" "$work/repo" 2>/dev/null || { mkdir -p "$work/repo"; cd "$work/repo"; git init --quiet -b ci-logs; git remote add origin "$url"; }
cd "$work/repo"
git config user.name "ci-logs"; git config user.email "ci-logs@users.noreply.github.com"
mkdir -p shots && cp "$dir"/*.png shots/ 2>/dev/null
git add shots && git commit --quiet -m "shots ${GITHUB_SHA:-}" || exit 0
git push --quiet origin ci-logs || git push --quiet --force origin ci-logs || true
