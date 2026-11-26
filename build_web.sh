#!/bin/sh
# builds dist-web/ (index.html + outonparole.wasm + mq_js_bundle.js). Needs: rustup target add wasm32-unknown-unknown
set -e
cargo build --release --target wasm32-unknown-unknown
mkdir -p dist-web
cp target/wasm32-unknown-unknown/release/outonparole.wasm dist-web/
cp web/index.html dist-web/
[ -f dist-web/mq_js_bundle.js ] || curl -fL -o dist-web/mq_js_bundle.js https://raw.githubusercontent.com/not-fl3/macroquad/master/js/mq_js_bundle.js
echo "serve with:  python3 -m http.server -d dist-web 8000"
