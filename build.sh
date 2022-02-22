#!/bin/bash
set -e

if [ -d "res" ]; then
  echo ""
else
  mkdir res
fi

RUSTFLAGS='-C link-arg=-s' cargo +stable build --target wasm32-unknown-unknown --release

cp ./target/wasm32-unknown-unknown/release/vault_contract.wasm ./res/

