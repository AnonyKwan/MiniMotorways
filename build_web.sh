#!/usr/bin/env bash
# Builds the browser version into dist/. Serve that folder with any static web server.
set -euo pipefail
cd "$(dirname "$0")"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist
cp web/index.html web/gl.js dist/
cp target/wasm32-unknown-unknown/release/little-lanes.wasm dist/
echo "Built dist/ ($(du -h dist/little-lanes.wasm | cut -f1) wasm). Try: python3 -m http.server -d dist 8000"
