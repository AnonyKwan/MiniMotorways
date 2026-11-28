#!/usr/bin/env bash
# Assembles dist/ from the web files and the compiled WebAssembly. Run after
#   cargo build --release --target wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf dist
mkdir -p dist
cp -r web/* dist/
cp target/wasm32-unknown-unknown/release/little_lanes_core.wasm dist/little_lanes_core.wasm
touch dist/.nojekyll
echo "dist/ ready: $(du -h dist/little_lanes_core.wasm | cut -f1) of wasm"
