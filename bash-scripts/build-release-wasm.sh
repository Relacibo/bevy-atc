#!/bin/sh
RUSTFLAGS='--cfg getrandom_backend="wasm_js" -C opt-level=z' \
cargo build \
  --release \
  --target wasm32-unknown-unknown || exit 1
wasm-bindgen \
  --no-typescript \
  --target web \
  --out-dir ./dist/ \
  --out-name "bevy-floppy" \
  ./target/wasm32-unknown-unknown/release/bevy-floppy.wasm || exit 1
