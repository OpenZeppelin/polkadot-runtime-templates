use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader},
    path::Path,
};

use serde::{de, Deserialize};
use sp_core::H160;

use self::{
    defaults::{entrypoint_address, entrypoint_contract},
    forge::ForgeContract,
    hardhat::HardhatContract,
};

mod abi;
mod defaults;
mod forge;
mod hardhat;

#[derive(Deserialize)]
pub struct ContractMetadata {
    pub address: H160,
    pub filename: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Contract {
    Hardhat(HardhatContract),
    Forge(ForgeContract),
}

impl Contract {
    pub fn bytecode(self) -> Vec<u8> {
        match self {
            Self::Hardhat(c) => c.bytecode,
            Self::Forge(c) => c.bytecode.object,
        }
    }
}

#[derive(Debug)]
pub enum ContractParsingError {
    #[allow(dead_code)]
    MetadataReadError(io::Error),
    #[allow(dead_code)]
    MetadataParseError(serde_json::Error),
}

#[derive(Default)]
pub enum ContractsPath {
    None,
    #[default]
    Default,
    Some(String),
}

pub fn parse_contracts(
    path: ContractsPath,
) -> Result<HashMap<H160, Contract>, ContractParsingError> {
    let mut res = HashMap::new();
    let path = match path {
        ContractsPath::None => return Ok(res),
        ContractsPath::Default => {
            res.insert(entrypoint_address(), entrypoint_contract());
            return Ok(res);
        }
        ContractsPath::Some(path) => path,
    };
    let path = Path::new(&path);
    let metadata_path = path.join(Path::new("contracts.json"));
    let metadata_file =
        File::open(metadata_path.as_path()).map_err(ContractParsingError::MetadataReadError)?;
    let metadata_reader = BufReader::new(metadata_file);
    let metadatas: Vec<ContractMetadata> = serde_json::from_reader(metadata_reader)
        .map_err(ContractParsingError::MetadataParseError)?;
    for metadata in metadatas {
        let ContractMetadata { filename, address } = metadata;
        let contract_path = path.join(Path::new(&filename));
        let Ok(contract_file) = File::open(contract_path.as_path()).map_err(|e| {
            println!("File {filename} can't be opened, skipping: {e:?}");
        }) else {
            continue;
        };
        let contract_reader = BufReader::new(contract_file);
        let Ok(contract) = serde_json::from_reader(contract_reader).map_err(|e| {
            println!("File {filename} can't be parsed, skipping: {e:?}");
        }) else {
            continue;
        };
        res.insert(address, contract);
    }
    Ok(res)
}

fn deserialize_bytecode<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    hex::decode(&s[2..]).map_err(|e| de::Error::custom(e.to_string()))
}
