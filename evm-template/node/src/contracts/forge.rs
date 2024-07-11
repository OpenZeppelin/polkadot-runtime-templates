use serde::Deserialize;

use super::{abi::AbiItem, deserialize_bytecode};

#[derive(Deserialize)]
pub struct ForgeContract {
    #[allow(dead_code)]
    pub abi: Vec<AbiItem>,
    pub bytecode: ForgeBytecode,
}

#[derive(Deserialize)]
pub struct ForgeBytecode {
    #[serde(deserialize_with = "deserialize_bytecode")]
    pub object: Vec<u8>,
}
