#!/bin/bash
rustup target add wasm32-unknown-unknown
rustup component add rust-src
cargo install ziggy cargo-afl honggfuzz grcov
AFL_SKIP_CPUFREQ=true AFL_I_DONT_CARE_ABOUT_MISSING_CRASHES=true cargo ziggy fuzz -t 20 -j 5