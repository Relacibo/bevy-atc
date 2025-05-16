#!/bin/sh
RUSTFLAGS='--cfg getrandom_backend="wasm_js" -C opt-level=z -C lto=thin -C embed-bitcode=yes' cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --target web \
  --out-dir ./dist/ \
  --out-name "bevy-floppy" \
  ./target/wasm32-unknown-unknown/release/bevy-floppy.wasm
cp -r ./assets ./dist/
cp -r ./public/* ./dist/
