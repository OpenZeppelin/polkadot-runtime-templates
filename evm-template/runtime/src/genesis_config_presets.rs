//! Genesis presets adapted for the EVM template (AccountId20 / H160).

use alloc::{format, vec, vec::Vec};

use cumulus_primitives_core::ParaId;
use frame_support::build_struct_json_patch;
use parachains_common::AuraId;
use serde_json::Value;
use sp_core::{ecdsa, Pair, Public};
use sp_genesis_builder::PresetId;
use sp_runtime::traits::{IdentifyAccount, Verify};

use crate::{
    constants::currency::EXISTENTIAL_DEPOSIT, AccountId, BalancesConfig, CollatorSelectionConfig,
    ParachainInfoConfig, PolkadotXcmConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys,
    Signature, SudoConfig,
};

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Parachain id used for genesis config presets of parachain template.
#[docify::export_content]
pub const PARACHAIN_ID: u32 = 1000;

/// Generate the session keys from individual elements.
pub fn template_session_keys(keys: AuraId) -> SessionKeys {
    SessionKeys { aura: keys }
}

/* ---------- Helpers brought over from node/chain_spec.rs ---------- */

/// Helper: produce a public key from seed (e.g., "Alice", "Bob").
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Alias to the runtime’s MultiSignature signer type.
type AccountPublic = <Signature as Verify>::Signer;

/// Helper: turn a seed into an AccountId20 (Ethereum-style address).
fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper: collator session key (Aura) from seed.
fn get_collator_keys_from_seed(seed: &str) -> AuraId {
    get_from_seed::<AuraId>(seed)
}

/* ---------- End helpers ---------- */

fn testnet_genesis(
    invulnerables: Vec<(AccountId, AuraId)>,
    endowed_accounts: Vec<AccountId>,
    root: AccountId,
    id: ParaId,
) -> Value {
    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1u128 << 60))
                .collect::<Vec<_>>(),
        },
        parachain_info: ParachainInfoConfig { parachain_id: id },
        collator_selection: CollatorSelectionConfig {
            invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
            candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
        },
        session: SessionConfig {
            keys: invulnerables
                .into_iter()
                .map(|(acc, aura)| {
                    (
                        acc.clone(),                 // account id (AccountId20)
                        acc,                         // validator id (AccountId20)
                        template_session_keys(aura), // session keys
                    )
                })
                .collect::<Vec<_>>(),
        },
        polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
        sudo: SudoConfig { key: Some(root) },
    })
}

fn local_testnet_genesis() -> Value {
    use hex_literal::hex;

    // Collators: use ECDSA seeds -> AccountId20, and Aura keys from seed.
    let invulnerables = vec![
        (get_account_id_from_seed::<ecdsa::Public>("Alice"), get_collator_keys_from_seed("Alice")),
        (get_account_id_from_seed::<ecdsa::Public>("Bob"), get_collator_keys_from_seed("Bob")),
    ];

    // Endowed accounts: the standard EVM dev addresses (Alith, Baltathar, Charleth, Dorothy, Ethan),
    // matching the node chain spec you pasted.
    let endowed_accounts = vec![
        AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
        AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")),
        AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")),
        AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")),
        AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")),
    ];

    // Sudo: Alith.
    let root = AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"));

    testnet_genesis(invulnerables, endowed_accounts, root, PARACHAIN_ID.into())
}

fn development_config_genesis() -> Value {
    // Mirror local_testnet for dev.
    local_testnet_genesis()
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_testnet_genesis(),
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}
