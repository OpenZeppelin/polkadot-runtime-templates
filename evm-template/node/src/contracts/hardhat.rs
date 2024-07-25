use serde::Deserialize;

use super::{abi::AbiItem, deserialize_bytecode};

#[derive(Deserialize)]
pub struct HardhatContract {
    #[allow(dead_code)]
    pub abi: Vec<AbiItem>,
    #[serde(deserialize_with = "deserialize_bytecode")]
    pub bytecode: Vec<u8>,
}
