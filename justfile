dev-wasm:
    RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo run --target wasm32-unknown-unknown -- 

build-release-wasm:
    ./bash-scripts/build-release-wasm.sh

astro-dev:
    cd astro && astro dev
