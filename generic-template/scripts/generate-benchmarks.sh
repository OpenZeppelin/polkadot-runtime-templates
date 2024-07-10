#!/bin/bash

set -e

while getopts f: flag
do
    case "${flag}" in
        f) filename=${OPTARG};;
    esac
done

cargo build --release --features=runtime-benchmarks

mkdir -p benchmarking/new-benchmarks

while IFS= read -r line; do
    echo "Creating benchmark for: $line"
    target/release/parachain-template-node benchmark pallet --steps=50 --repeat=20 --extrinsic=* --wasm-execution=compiled --heap-pages=4096 --json-file=benchmarking/results/results-$line.json --pallet=$line --chain=dev --output=benchmarking/new-benchmarks/$line.rs
done < $filename