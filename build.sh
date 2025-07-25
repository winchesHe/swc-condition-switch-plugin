#!/bin/bash
set -e

echo "Building SWC Condition Plugin..."

rustup target add wasm32-wasip1

cargo clean

cargo build --target wasm32-wasip1 --release

cp target/wasm32-wasip1/release/swc_condition_plugin.wasm ./swc_condition_plugin.wasm

echo "Build completed! Plugin available at: swc_condition_plugin.wasm"
echo "File size: $(ls -lh swc_condition_plugin.wasm | awk '{print $5}')"
