#!/usr/bin/env bash
# Publishes the tail of a CI log to the `ci-logs` branch so it can be read without signing in to GitHub.
# usage: push_log.sh <name> <logfile>      (needs GITHUB_TOKEN and GITHUB_REPOSITORY)
set -u
name="$1"; file="$2"
[ -f "$file" ] || exit 0
work="$(mktemp -d)"
url="https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
if git clone --quiet --depth 1 --branch ci-logs "$url" "$work/repo" 2>/dev/null; then
  cd "$work/repo"
else
  mkdir -p "$work/repo" && cd "$work/repo"
  git init --quiet -b ci-logs
  git remote add origin "$url"
fi
git config user.name "ci-logs"
git config user.email "ci-logs@users.noreply.github.com"
{
  echo "commit: ${GITHUB_SHA:-unknown}  ref: ${GITHUB_REF_NAME:-?}  run: ${GITHUB_RUN_ID:-?}"
  echo "-----"
  tail -n 400 "$file"
} > "${name}.log"
git add "${name}.log"
git commit --quiet -m "log ${name} ${GITHUB_SHA:-}" || exit 0
git push --quiet origin ci-logs || git push --quiet --force origin ci-logs || true
