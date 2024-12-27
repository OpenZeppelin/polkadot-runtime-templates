use std::collections::BTreeMap;

use cumulus_primitives_core::ParaId;
use evm_runtime_template::{AccountId, OpenZeppelinPrecompiles as Precompiles, Runtime};
#[cfg(not(feature = "tanssi"))]
use evm_runtime_template::{AuraId, Signature};
use fp_evm::GenesisAccount;
use hex_literal::hex;
use log::error;
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::H160;
#[cfg(not(feature = "tanssi"))]
use sp_core::{ecdsa, Pair, Public};
#[cfg(not(feature = "tanssi"))]
use sp_runtime::traits::{IdentifyAccount, Verify};

use crate::contracts::{parse_contracts, ContractsPath};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
#[cfg(not(feature = "tanssi"))]
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
    /// The relay chain of the Parachain.
    pub relay_chain: String,
    /// The id of the Parachain.
    pub para_id: u32,
}

impl Extensions {
    /// Try to get the extension from the given `ChainSpec`.
    pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
        sc_chain_spec::get_extension(chain_spec.extensions())
    }
}

#[cfg(not(feature = "tanssi"))]
type AccountPublic = <Signature as Verify>::Signer;

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain
/// in tuple format.
#[cfg(not(feature = "tanssi"))]
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
    get_from_seed::<AuraId>(seed)
}

/// Helper function to generate an account ID from seed
#[cfg(not(feature = "tanssi"))]
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we
/// have just one key).
#[cfg(not(feature = "tanssi"))]
pub fn template_session_keys(keys: AuraId) -> evm_runtime_template::SessionKeys {
    evm_runtime_template::SessionKeys { aura: keys }
}

pub fn development_config(contracts_path: ContractsPath) -> ChainSpec {
    // Give your base currency a unit name and decimal places
    let mut properties = sc_chain_spec::Properties::new();
    properties.insert("tokenSymbol".into(), "UNIT".into());
    properties.insert("tokenDecimals".into(), 12.into());
    properties.insert("ss58Format".into(), 42.into());
    properties.insert("isEthereum".into(), true.into());
    // This is very important for us, it lets us track the usage of our templates, and have no downside for the node/runtime. Please do not remove :)
    properties.insert("basedOn".into(), "OpenZeppelin EVM Template".into());

    ChainSpec::builder(
        evm_runtime_template::WASM_BINARY.expect("WASM binary was not built, please build it!"),
        Extensions {
            relay_chain: "paseo-local".into(),
            // You MUST set this to the correct network!
            para_id: 1000,
        },
    )
    .with_name("Development")
    .with_id("dev")
    .with_chain_type(ChainType::Development)
    .with_genesis_config_patch(testnet_genesis(
        // initial collators.
        #[cfg(not(feature = "tanssi"))]
        vec![
            (
                get_account_id_from_seed::<ecdsa::Public>("Alice"),
                get_collator_keys_from_seed("Alice"),
            ),
            (get_account_id_from_seed::<ecdsa::Public>("Bob"), get_collator_keys_from_seed("Bob")),
        ],
        vec![
            // Alith, Baltathar, Charleth and Dorothy, Ethan
            AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
            AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")),
            AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")),
            AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")),
            AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")),
        ],
        // Alith
        AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
        1000.into(),
        contracts_path,
    ))
    .with_properties(properties)
    .build()
}

pub fn local_testnet_config(contracts_path: ContractsPath) -> ChainSpec {
    // Give your base currency a unit name and decimal places
    let mut properties = sc_chain_spec::Properties::new();
    properties.insert("tokenSymbol".into(), "UNIT".into());
    properties.insert("tokenDecimals".into(), 12.into());
    properties.insert("ss58Format".into(), 42.into());
    properties.insert("isEthereum".into(), true.into());

    #[allow(deprecated)]
    ChainSpec::builder(
        evm_runtime_template::WASM_BINARY.expect("WASM binary was not built, please build it!"),
        Extensions {
            relay_chain: "paseo-local".into(),
            // You MUST set this to the correct network!
            para_id: 1000,
        },
    )
    .with_name("Local Testnet")
    .with_id("local_testnet")
    .with_chain_type(ChainType::Local)
    .with_genesis_config_patch(testnet_genesis(
        // initial collators.
        #[cfg(not(feature = "tanssi"))]
        vec![
            (
                get_account_id_from_seed::<ecdsa::Public>("Alice"),
                get_collator_keys_from_seed("Alice"),
            ),
            (get_account_id_from_seed::<ecdsa::Public>("Bob"), get_collator_keys_from_seed("Bob")),
        ],
        vec![
            // Alith, Baltathar, Charleth and Dorothy, Ethan
            AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
            AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")),
            AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")),
            AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")),
            AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")),
        ],
        // Alith
        AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
        1000.into(),
        contracts_path,
    ))
    .with_protocol_id("template-local")
    .with_properties(properties)
    .build()
}

#[cfg(not(feature = "tanssi"))]
fn testnet_genesis(
    invulnerables: Vec<(AccountId, AuraId)>,
    #[cfg(not(feature = "runtime-benchmarks"))] endowed_accounts: Vec<AccountId>,
    #[cfg(feature = "runtime-benchmarks")] mut endowed_accounts: Vec<AccountId>,
    root: AccountId,
    id: ParaId,
    contracts_path: ContractsPath,
) -> serde_json::Value {
    let contracts = parse_contracts(contracts_path)
        .map_err(|e| error!("Error while parsing contracts: {e:?}"))
        .unwrap_or_default();
    let precompiles = Precompiles::<Runtime>::used_addresses()
        .map(|addr| {
            (
                addr,
                GenesisAccount {
                    nonce: Default::default(),
                    balance: Default::default(),
                    storage: Default::default(),
                    // bytecode to revert without returning data
                    // (PUSH1 0x00 PUSH1 0x00 REVERT)
                    code: vec![0x60, 0x00, 0x60, 0x00, 0xFD],
                },
            )
        })
        .into_iter();
    let accounts: BTreeMap<H160, GenesisAccount> = contracts
        .into_iter()
        .map(|(address, contract)| {
            (
                address,
                GenesisAccount {
                    code: contract.bytecode(),
                    nonce: Default::default(),
                    balance: Default::default(),
                    storage: Default::default(),
                },
            )
        })
        .chain(precompiles)
        .collect();
    let candidacy_bond: u64 = 1_000_000_000 * 16;
    #[cfg(feature = "runtime-benchmarks")]
    endowed_accounts.push(AccountId::from(hex!("1000000000000000000000000000000000000001")));
    serde_json::json!({
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|k| (k, 1u64 << 60)).collect::<Vec<_>>(),
        },
        "parachainInfo": {
            "parachainId": id,
        },
        "collatorSelection": {
            "invulnerables": invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
            "candidacyBond": candidacy_bond,
        },
        "session": {
            "keys": invulnerables
                .into_iter()
                .map(|(acc, aura)| {
                    (
                        acc,                        // account id
                        acc,                         // validator id
                        template_session_keys(aura), // session keys
                    )
                })
            .collect::<Vec<_>>(),
        },
        "treasury": {},
        "evmChainId": {
            "chainId": 9999
        },
        "evm": {
            "accounts": accounts
        },
        "polkadotXcm": {
            "safeXcmVersion": Some(SAFE_XCM_VERSION),
        },
        "sudo": { "key": Some(root) }
    })
}

#[cfg(feature = "tanssi")]
fn testnet_genesis(
    #[cfg(not(feature = "runtime-benchmarks"))] endowed_accounts: Vec<AccountId>,
    #[cfg(feature = "runtime-benchmarks")] mut endowed_accounts: Vec<AccountId>,
    root: AccountId,
    id: ParaId,
    contracts_path: ContractsPath,
) -> serde_json::Value {
    let contracts = parse_contracts(contracts_path)
        .map_err(|e| error!("Error while parsing contracts: {e:?}"))
        .unwrap_or_default();
    let precompiles = Precompiles::<Runtime>::used_addresses()
        .map(|addr| {
            (
                addr,
                GenesisAccount {
                    nonce: Default::default(),
                    balance: Default::default(),
                    storage: Default::default(),
                    // bytecode to revert without returning data
                    // (PUSH1 0x00 PUSH1 0x00 REVERT)
                    code: vec![0x60, 0x00, 0x60, 0x00, 0xFD],
                },
            )
        })
        .into_iter();
    let accounts: BTreeMap<H160, GenesisAccount> = contracts
        .into_iter()
        .map(|(address, contract)| {
            (
                address,
                GenesisAccount {
                    code: contract.bytecode(),
                    nonce: Default::default(),
                    balance: Default::default(),
                    storage: Default::default(),
                },
            )
        })
        .chain(precompiles)
        .collect();
    #[cfg(feature = "runtime-benchmarks")]
    endowed_accounts.push(AccountId::from(hex!("1000000000000000000000000000000000000001")));
    serde_json::json!({
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|k| (k, 1u64 << 60)).collect::<Vec<_>>(),
        },
        "parachainInfo": {
            "parachainId": id,
        },
        "treasury": {},
        "evmChainId": {
            "chainId": 9999
        },
        "evm": {
            "accounts": accounts
        },
        "polkadotXcm": {
            "safeXcmVersion": Some(SAFE_XCM_VERSION),
        },
        "sudo": { "key": Some(root) }
    })
}
