//! Initialize FHE Server

use once_cell::sync::OnceCell;
use tfhe::{generate_keys, set_server_key, ConfigBuilder, ServerKey};

// Optionally keep the server key around if you want to inspect it / route later.
static SERVER_KEY: OnceCell<ServerKey> = OnceCell::new();

pub fn init_fhe() {
    // For a quick start, just generate keys on boot.
    // (In production, load from disk or KMS; the helpers are pure, this I/O is fine here.)
    let config = ConfigBuilder::default().build();
    let (_client_key, server_key) = generate_keys(config);

    set_server_key(server_key.clone()); // makes it globally usable by tfhe-rs
    SERVER_KEY.set(server_key).ok(); // keep a copy if you’ll add routing later
}
