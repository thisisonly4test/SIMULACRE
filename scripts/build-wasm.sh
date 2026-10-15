#!/usr/bin/env sh
# Builds the in-browser engine into web/pkg/ (committed, so static hosting
# needs no Rust toolchain). Requires the wasm32-unknown-unknown target and
# wasm-bindgen-cli at the same version as the wasm-bindgen crate:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <version in Cargo.lock>
set -eu
cd "$(dirname "$0")/.."
cargo build -p simulacre-wasm --target wasm32-unknown-unknown --profile wasm
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/wasm/simulacre_wasm.wasm
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o web/pkg/simulacre_wasm_bg.wasm web/pkg/simulacre_wasm_bg.wasm
fi
ls -l web/pkg
