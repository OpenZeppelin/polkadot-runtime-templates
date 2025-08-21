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
                        acc,                         // account id (AccountId20)
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

#[cfg(test)]
mod tests {
    // We use hex! only inside tests. The runtime already depends on hex-literal for this file.
    use hex_literal::hex;
    use serde_json::json;

    use super::*;

    /// Parse a preset bytes blob with serde_json.
    fn parse_bytes(b: Vec<u8>) -> Value {
        serde_json::from_slice::<Value>(&b).expect("preset must be valid JSON")
    }

    /// Serialize a runtime AccountId (AccountId20/H160) into JSON for comparison.
    fn account_json(acc: &AccountId) -> Value {
        serde_json::to_value(acc).expect("AccountId must serialize to JSON")
    }

    #[test]
    fn local_testnet_genesis_shape_and_contents() {
        let v = super::local_testnet_genesis();

        // Top-level keys present
        for k in
            ["balances", "parachainInfo", "collatorSelection", "session", "polkadotXcm", "sudo"]
        {
            assert!(v.get(k).is_some(), "{k} missing");
        }

        // ParachainId
        assert_eq!(v["parachainInfo"]["parachainId"], json!(PARACHAIN_ID), "wrong parachain id");

        // XCM version
        assert_eq!(
            v["polkadotXcm"]["safeXcmVersion"],
            json!(Some(SAFE_XCM_VERSION)),
            "wrong SAFE_XCM_VERSION"
        );

        // ----- Collators / invulnerables: must be [Alice, Bob] derived via ECDSA seeds -----
        let expected_alice = get_account_id_from_seed::<ecdsa::Public>("Alice");
        let expected_bob = get_account_id_from_seed::<ecdsa::Public>("Bob");
        let invuls = v["collatorSelection"]["invulnerables"]
            .as_array()
            .expect("collatorSelection.invulnerables must be an array");
        assert_eq!(invuls.len(), 2, "expected two invulnerables");
        assert_eq!(
            invuls[0],
            account_json(&expected_alice),
            "first invulnerable must be Alice (ECDSA)"
        );
        assert_eq!(
            invuls[1],
            account_json(&expected_bob),
            "second invulnerable must be Bob (ECDSA)"
        );

        // candidacyBond must be EXISTENTIAL_DEPOSIT * 16
        assert_eq!(
            v["collatorSelection"]["candidacyBond"],
            json!(EXISTENTIAL_DEPOSIT * 16),
            "wrong candidacy bond"
        );

        // ----- Session keys: one per invulnerable; controller == validator; contains 'aura' -----
        let sess = v["session"]["keys"].as_array().expect("session.keys must be an array");
        assert_eq!(sess.len(), invuls.len(), "session keys length must equal invulnerables length");
        for entry in sess {
            let arr = entry
                .as_array()
                .expect("each session entry must be a 3-tuple [acc, validator, keys]");
            assert_eq!(arr.len(), 3, "session entry must have 3 elements");
            assert_eq!(arr[0], arr[1], "controller and validator account must match");
            assert!(arr[2].get("aura").is_some(), "session keys must contain 'aura'");
        }

        // ----- Sudo key is Alith (EVM dev addr) -----
        let alith = AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"));
        assert_eq!(v["sudo"]["key"], json!(Some(account_json(&alith))), "sudo key must be Alith");

        // ----- Endowed accounts: the 5 standard EVM dev addrs -----
        let expected_endowed: Vec<AccountId> = vec![
            AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")), // Alith
            AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
            AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
            AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")), // Dorothy
            AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")), // Ethan
        ];

        let balances =
            v["balances"]["balances"].as_array().expect("balances.balances must be an array");

        // Length check
        assert_eq!(balances.len(), expected_endowed.len(), "endowed accounts length mismatch");

        // Helper: turn a JSON account value (usually a hex string) into a comparable String.
        fn json_acc_to_string(val: &Value) -> String {
            // Expect string form (e.g., "0xf24f..."). If not, fall back to JSON rendering.
            val.as_str().map(|s| s.to_owned()).unwrap_or_else(|| val.to_string())
        }

        // Extract the set of endowed accounts from the JSON
        use std::collections::BTreeSet;
        let got: BTreeSet<String> = balances
            .iter()
            .map(|pair| {
                let arr = pair.as_array().expect("[account, amount] pair");
                assert_eq!(arr.len(), 2, "balance entry must be [account, amount]");
                json_acc_to_string(&arr[0])
            })
            .collect();

        // Build the expected set from our AccountId list
        let want: BTreeSet<String> =
            expected_endowed.iter().map(|a| json_acc_to_string(&account_json(a))).collect();

        assert_eq!(got, want, "endowed account set mismatch");
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

    #[test]
    fn invulnerables_match_ecdsa_seed_derivation() {
        // Ensure the addresses inside the JSON exactly match the ones derived from ECDSA seeds.
        let v = super::local_testnet_genesis();
        let invuls = v["collatorSelection"]["invulnerables"]
            .as_array()
            .expect("collatorSelection.invulnerables must be an array");

        let alice = get_account_id_from_seed::<ecdsa::Public>("Alice");
        let bob = get_account_id_from_seed::<ecdsa::Public>("Bob");

        assert_eq!(invuls[0], account_json(&alice), "Alice derivation mismatch");
        assert_eq!(invuls[1], account_json(&bob), "Bob derivation mismatch");
    }
}
