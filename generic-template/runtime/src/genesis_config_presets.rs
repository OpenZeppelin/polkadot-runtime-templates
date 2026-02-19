use alloc::{vec, vec::Vec};

use cumulus_primitives_core::ParaId;
use frame_support::build_struct_json_patch;
use parachains_common::AuraId;
use serde_json::Value;
use sp_genesis_builder::PresetId;
use sp_keyring::Sr25519Keyring;

use crate::{
    constants::currency::EXISTENTIAL_DEPOSIT, AccountId, BalancesConfig, ParachainInfoConfig,
    PolkadotXcmConfig, RuntimeGenesisConfig, SessionKeys, SudoConfig,
};
#[cfg(not(feature = "tanssi"))]
use crate::{CollatorSelectionConfig, SessionConfig};

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;
/// Parachain id used for genesis config presets of parachain template.
#[docify::export_content]
pub const PARACHAIN_ID: u32 = 1000;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
#[cfg(not(feature = "tanssi"))]
pub fn template_session_keys(keys: AuraId) -> SessionKeys {
    SessionKeys { aura: keys }
}

#[cfg(not(feature = "tanssi"))]
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
                        acc.clone(),                 // account id
                        acc,                         // validator id
                        template_session_keys(aura), // session keys
                    )
                })
                .collect::<Vec<_>>(),
        },
        polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
        sudo: SudoConfig { key: Some(root) },
    })
}

#[cfg(feature = "tanssi")]
fn testnet_genesis(
    _invulnerables: Vec<(AccountId, AuraId)>,
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
        polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
        sudo: SudoConfig { key: Some(root) },
    })
}

fn local_testnet_genesis() -> Value {
    testnet_genesis(
        // initial collators.
        vec![
            (Sr25519Keyring::Alice.to_account_id(), Sr25519Keyring::Alice.public().into()),
            (Sr25519Keyring::Bob.to_account_id(), Sr25519Keyring::Bob.public().into()),
        ],
        Sr25519Keyring::well_known().map(|k| k.to_account_id()).collect(),
        Sr25519Keyring::Alice.to_account_id(),
        PARACHAIN_ID.into(),
    )
}

fn development_config_genesis() -> Value {
    testnet_genesis(
        // initial collators.
        vec![
            (Sr25519Keyring::Alice.to_account_id(), Sr25519Keyring::Alice.public().into()),
            (Sr25519Keyring::Bob.to_account_id(), Sr25519Keyring::Bob.public().into()),
        ],
        Sr25519Keyring::well_known().map(|k| k.to_account_id()).collect(),
        Sr25519Keyring::Alice.to_account_id(),
        PARACHAIN_ID.into(),
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<vec::Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Parse a preset bytes blob with serde_json.
    fn parse_bytes(b: Vec<u8>) -> Value {
        serde_json::from_slice::<Value>(&b).expect("preset must be valid JSON")
    }

    /// Extract a JSON value for an AccountId by serializing the runtime AccountId directly.
    /// This avoids guessing about how AccountId32 is encoded (hex string with 0x…).
    fn account_json(acc: &AccountId) -> Value {
        serde_json::to_value(acc).expect("AccountId must serialize to JSON")
    }

    #[test]
    fn local_testnet_genesis_shape_is_reasonable() {
        let v = super::local_testnet_genesis();

        // Top-level keys we care about exist
        assert!(v.get("balances").is_some(), "balances missing");
        assert!(v.get("parachainInfo").is_some(), "parachainInfo missing");
        assert!(v.get("collatorSelection").is_some(), "collatorSelection missing");
        assert!(v.get("session").is_some(), "session missing");
        assert!(v.get("polkadotXcm").is_some(), "polkadotXcm missing");
        assert!(v.get("sudo").is_some(), "sudo missing");

        // parachainId
        assert_eq!(v["parachainInfo"]["parachainId"], json!(PARACHAIN_ID), "wrong parachain id");

        // XCM version
        assert_eq!(
            v["polkadotXcm"]["safeXcmVersion"],
            json!(Some(SAFE_XCM_VERSION)),
            "wrong SAFE_XCM_VERSION"
        );

        // Endowed accounts length should match well_known()
        let expected_endowed: Vec<AccountId> =
            Sr25519Keyring::well_known().map(|k| k.to_account_id()).collect();
        let balances =
            v["balances"]["balances"].as_array().expect("balances.balances must be an array");
        assert_eq!(balances.len(), expected_endowed.len(), "endowed accounts length mismatch");

        // Sudo key must be Alice
        let alice = Sr25519Keyring::Alice.to_account_id();
        assert_eq!(v["sudo"]["key"], json!(Some(account_json(&alice))), "sudo key is not Alice");

        // Collators (invulnerables) must be Alice and Bob
        let invuls = v["collatorSelection"]["invulnerables"]
            .as_array()
            .expect("collatorSelection.invulnerables must be an array");
        assert_eq!(invuls.len(), 2, "expected two invulnerables");
        let expected_alice = account_json(&Sr25519Keyring::Alice.to_account_id());
        let expected_bob = account_json(&Sr25519Keyring::Bob.to_account_id());
        assert_eq!(invuls[0], expected_alice, "first invulnerable must be Alice");
        assert_eq!(invuls[1], expected_bob, "second invulnerable must be Bob");

        // candidacyBond must be EXISTENTIAL_DEPOSIT * 16
        assert_eq!(
            v["collatorSelection"]["candidacyBond"],
            json!(EXISTENTIAL_DEPOSIT * 16),
            "wrong candidacy bond"
        );

        // Session keys: one entry per invulnerable; controller == validator
        let sess = v["session"]["keys"].as_array().expect("session.keys must be an array");
        assert_eq!(sess.len(), invuls.len(), "session keys length must equal invulnerables length");
        for entry in sess {
            let arr = entry
                .as_array()
                .expect("each session entry must be a 3-tuple [acc, validator, keys]");
            assert_eq!(arr.len(), 3, "session entry must have 3 elements");
            assert_eq!(arr[0], arr[1], "controller and validator account must match");
            // We don't assert on the actual Aura key bytes here—just presence/shape.
            assert!(arr[2].get("aura").is_some(), "session keys must contain aura");
        }
    }

    #[test]
    fn development_genesis_mirrors_local_testnet() {
        // In this template, development == local_testnet (per implementation).
        let dev = super::development_config_genesis();
        let local = super::local_testnet_genesis();
        assert_eq!(dev, local, "development config should mirror local_testnet");
    }

    #[test]
    fn get_preset_roundtrips_known_ids() {
        // LOCAL_TESTNET
        let p =
            super::get_preset(&PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET))
                .expect("local preset should exist");
        let from_api = parse_bytes(p);
        let from_fn = super::local_testnet_genesis();
        assert_eq!(from_api, from_fn, "local_testnet preset must match function output");

        // DEV
        let p = super::get_preset(&PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET))
            .expect("dev preset should exist");
        let from_api = parse_bytes(p);
        let from_fn = super::development_config_genesis();
        assert_eq!(from_api, from_fn, "dev preset must match function output");
    }

    #[test]
    fn preset_names_lists_supported() {
        let names = super::preset_names();
        assert!(
            names.contains(&PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET)),
            "DEV preset should be listed"
        );
        assert!(
            names.contains(&PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)),
            "LOCAL_TESTNET preset should be listed"
        );
    }
}
