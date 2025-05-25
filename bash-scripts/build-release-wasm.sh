#!/bin/sh
export PACKAGE_NAME="bevy-atc"
export RUSTFLAGS='--cfg getrandom_backend="wasm_js" -C opt-level=z'
cargo build \
  --release \
  --target wasm32-unknown-unknown || exit 1
wasm-bindgen \
  --no-typescript \
  --target web \
  --out-dir ./dist/ \
  --out-name "$PACKAGE_NAME" \
  ./target/wasm32-unknown-unknown/release/$PACKAGE_NAME.wasm || exit 1
