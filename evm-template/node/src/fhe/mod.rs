//! Initialize FHE Server

// #![cfg(all(feature = "fhe", test))]
// uncomment once working
// pub mod testing;

use once_cell::sync::OnceCell;
use tfhe::{generate_keys, set_server_key, ClientKey, ConfigBuilder, ServerKey};

static SERVER_KEY: OnceCell<ServerKey> = OnceCell::new();
static CLIENT_KEY: OnceCell<ClientKey> = OnceCell::new();

pub fn init() {
    // DEV ONLY: generate at boot. In prod, load from disk or KMS.
    let config = ConfigBuilder::default().build();
    let (ck, sk) = generate_keys(config);

    set_server_key(sk.clone());
    SERVER_KEY.set(sk).ok();
    CLIENT_KEY.set(ck).ok();
}

pub fn client_key() -> &'static ClientKey {
    CLIENT_KEY.get().expect("ClientKey not initialized")
}
