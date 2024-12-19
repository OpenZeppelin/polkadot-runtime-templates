#[cfg(all(not(feature = "metadata-hash"), feature = "std"))]
fn main() {
    substrate_wasm_builder::WasmBuilder::build_using_defaults();
}

#[cfg(all(feature = "metadata-hash", feature = "std"))]
fn main() {
    substrate_wasm_builder::WasmBuilder::init_with_defaults()
        .enable_metadata_hash("UNIT", 12)
        .build();
}

/// The wasm builder is deactivated when compiling
/// this crate for wasm to speed up the compilation.
#[cfg(not(feature = "std"))]
fn main() {}
