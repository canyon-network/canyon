#!/usr/bin/env bash

set -e

cd "$(dirname "$0")"

cd ..

cargo build --release --features=runtime-benchmarks

bin=./target/release/canyon

pallets=(
  poa
)

for pallet in "${pallets[@]}"; do
  output="./pallets/$pallet/src/weights.rs"

  "$bin" benchmark \
    --chain "dev" \
    --execution=wasm \
    --wasm-execution=compiled \
    --pallet "pallet_$pallet" \
    --extrinsic "*" \
    --steps=50 \
    --heap-pages=4096 \
    --repeat 20 \
    --template=./scripts/pallet-weights-template.hbs \
    --output="$output"

  rustfmt "$output"
done
