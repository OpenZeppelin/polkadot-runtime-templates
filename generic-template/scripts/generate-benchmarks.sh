#!/bin/bash

set -e

while getopts f:s:r: flag
do
    case "${flag}" in
        f) filename=${OPTARG};;
        s) steps=${OPTARG};;
        r) rep=${OPTARG};;
    esac
done

cargo build --release --features=runtime-benchmarks

mkdir -p benchmarking/new-benchmarks
mkdir -p benchmarking/results

while IFS= read -r line; do
    echo "Creating benchmark for: $line"
    target/release/parachain-template-node benchmark pallet --steps=$steps --repeat=$rep --extrinsic=* --wasm-execution=compiled --heap-pages=4096 --json-file=benchmarking/results/results-$line.json --pallet=$line --chain=dev --output=benchmarking/new-benchmarks/$line.rs
done < $filename