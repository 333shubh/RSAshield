#!/usr/bin/env bash
# Build the in-browser engine (web/rsashield.wasm) used by the hosted website.
# Needs the wasm32 target once:  rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib
cp target/wasm32-unknown-unknown/release/rsashield.wasm web/rsashield.wasm
echo "wrote web/rsashield.wasm ($(wc -c < web/rsashield.wasm) bytes)"
