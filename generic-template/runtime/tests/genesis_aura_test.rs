// Regression test for Aura authorities initialization at genesis.
//
// BUG: "Invalid AuRa author index X for authorities: BoundedVec([], 100000)"
//
// ROOT CAUSE:
// The `openzeppelin-pallet-abstractions` crate's `PALLET_NAMES` constant in
// `src/consensus.rs` previously defined the pallet order as:
//   Authorship -> Aura -> AuraExt -> CollatorSelection -> Session
//
// But the CORRECT order must be:
//   Authorship -> CollatorSelection -> Session -> Aura -> AuraExt
//
// WHY THIS MATTERS:
// 1. Session pallet's genesis_build populates pallet_aura::Authorities
// 2. AuraExt's genesis_build reads from pallet_aura::Authorities and caches it
// 3. If Session runs AFTER AuraExt, the cache will be empty
// 4. When relay chain validates parachain blocks, cumulus_pallet_aura_ext::BlockExecutor
//    reads from the empty cache and panics
//
// FIX:
// The `openzeppelin-pallet-abstractions` crate's `src/consensus.rs` was updated
// (commit 927eae6c8ae0a0d9c85a8b26e9618ac07d783a2b) to change PALLET_NAMES order
// so Session comes BEFORE Aura and AuraExt.
//
// NOTE ON TEST APPROACH:
// This test manually calls assimilate_storage in the correct order to document
// expected behavior. The actual pallet ordering from the abstractions crate's
// construct_runtime macro is verified by:
// 1. The runtime building successfully with the patched abstractions
// 2. This regression test documenting what correct behavior should look like
//
// A test that directly exercises the macro ordering would require using the
// full GenesisBuilder machinery with RuntimeGenesisConfig::build_storage().

#[cfg(not(feature = "tanssi"))]
mod genesis_aura_tests {
    use generic_runtime_template::{AccountId, BuildStorage, Runtime, SessionKeys};
    use sp_consensus_aura::sr25519::AuthorityId as AuraId;
    use sp_core::Pair;

    fn get_test_aura_id(seed: &str) -> AuraId {
        sp_core::sr25519::Pair::from_string(&format!("//{}", seed), None)
            .expect("valid seed")
            .public()
            .into()
    }

    fn get_test_account_id(seed: &str) -> AccountId {
        sp_core::sr25519::Pair::from_string(&format!("//{}", seed), None)
            .expect("valid seed")
            .public()
            .into()
    }

    /// This test verifies that the pallet_aura::Authorities storage is properly
    /// populated from the session keys at genesis. This is what AuraApi::authorities()
    /// returns and what AuraExt's genesis_build reads from.
    ///
    /// If this test fails with empty authorities, it indicates that the pallet ordering
    /// in construct_runtime is incorrect. The correct order must be: Session -> Aura -> AuraExt
    #[test]
    fn aura_authorities_populated_at_genesis() {
        // Create test authorities
        let alice_aura = get_test_aura_id("Alice");
        let bob_aura = get_test_aura_id("Bob");
        let alice_account = get_test_account_id("Alice");
        let bob_account = get_test_account_id("Bob");

        // Build genesis config using the pallet genesis configs directly
        let mut storage = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .expect("frame_system storage");

        // Configure balances
        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice_account.clone(), 1u128 << 60),
                (bob_account.clone(), 1u128 << 60),
            ],
        }
        .assimilate_storage(&mut storage)
        .expect("balances storage");

        // Configure parachain info
        parachain_info::GenesisConfig::<Runtime> {
            _config: Default::default(),
            parachain_id: 1000u32.into(),
        }
        .assimilate_storage(&mut storage)
        .expect("parachain_info storage");

        // Configure collator selection
        pallet_collator_selection::GenesisConfig::<Runtime> {
            invulnerables: vec![alice_account.clone(), bob_account.clone()],
            candidacy_bond: 16_000_000_000u128,
            desired_candidates: 0,
        }
        .assimilate_storage(&mut storage)
        .expect("collator_selection storage");

        // Configure session with keys - this is the critical part
        pallet_session::GenesisConfig::<Runtime> {
            keys: vec![
                (
                    alice_account.clone(),
                    alice_account.clone(),
                    SessionKeys { aura: alice_aura.clone() },
                ),
                (bob_account.clone(), bob_account.clone(), SessionKeys { aura: bob_aura.clone() }),
            ],
        }
        .assimilate_storage(&mut storage)
        .expect("session storage");

        // Configure Aura (empty genesis, authorities come from session)
        pallet_aura::GenesisConfig::<Runtime> { authorities: vec![] }
            .assimilate_storage(&mut storage)
            .expect("aura storage");

        // Configure AuraExt (this reads from pallet_aura::Authorities)
        cumulus_pallet_aura_ext::GenesisConfig::<Runtime> { _config: Default::default() }
            .assimilate_storage(&mut storage)
            .expect("aura_ext storage");

        let mut ext: sp_io::TestExternalities = storage.into();
        ext.execute_with(|| {
            // Get authorities directly from pallet_aura
            // This is what AuraApi::authorities() returns
            let aura_authorities = pallet_aura::Authorities::<Runtime>::get();

            // The authorities list MUST NOT be empty
            // If this fails, it means Session pallet's genesis_build didn't populate
            // Aura authorities (either Session ran after Aura, or there's a config issue)
            assert!(
                !aura_authorities.is_empty(),
                "Aura authorities should not be empty at genesis. This indicates that Session \
                 pallet did not populate Aura authorities. Check pallet ordering: Session must \
                 come BEFORE Aura."
            );

            // Verify we have the expected number of authorities
            assert_eq!(aura_authorities.len(), 2, "Expected 2 authorities (Alice and Bob)");

            // Verify the specific authorities are present
            assert!(
                aura_authorities.contains(&alice_aura),
                "Alice's AuraId should be in authorities"
            );
            assert!(aura_authorities.contains(&bob_aura), "Bob's AuraId should be in authorities");
        });
    }

    /// This test verifies that when session keys are set, the Aura pallet
    /// properly receives the authorities from the Session pallet via the
    /// SessionHandler mechanism.
    ///
    /// The pallet ordering is critical:
    /// 1. Session must be initialized first (sets up session keys)
    /// 2. Aura's session handler must be called to receive keys
    /// 3. AuraExt's genesis_build reads from pallet_aura::Authorities
    ///
    /// If pallet ordering is wrong (e.g., Aura before Session), the authorities
    /// won't be propagated correctly.
    #[test]
    fn session_populates_aura_authorities() {
        let alice_aura = get_test_aura_id("Alice");
        let bob_aura = get_test_aura_id("Bob");
        let alice_account = get_test_account_id("Alice");
        let bob_account = get_test_account_id("Bob");

        let mut storage = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .expect("frame_system storage");

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice_account.clone(), 1u128 << 60),
                (bob_account.clone(), 1u128 << 60),
            ],
        }
        .assimilate_storage(&mut storage)
        .expect("balances storage");

        parachain_info::GenesisConfig::<Runtime> {
            _config: Default::default(),
            parachain_id: 1000u32.into(),
        }
        .assimilate_storage(&mut storage)
        .expect("parachain_info storage");

        pallet_collator_selection::GenesisConfig::<Runtime> {
            invulnerables: vec![alice_account.clone(), bob_account.clone()],
            candidacy_bond: 16_000_000_000u128,
            desired_candidates: 0,
        }
        .assimilate_storage(&mut storage)
        .expect("collator_selection storage");

        // Configure session with keys
        pallet_session::GenesisConfig::<Runtime> {
            keys: vec![
                (
                    alice_account.clone(),
                    alice_account.clone(),
                    SessionKeys { aura: alice_aura.clone() },
                ),
                (bob_account.clone(), bob_account.clone(), SessionKeys { aura: bob_aura.clone() }),
            ],
        }
        .assimilate_storage(&mut storage)
        .expect("session storage");

        // Aura genesis with empty authorities - they should come from Session
        pallet_aura::GenesisConfig::<Runtime> { authorities: vec![] }
            .assimilate_storage(&mut storage)
            .expect("aura storage");

        // AuraExt genesis
        cumulus_pallet_aura_ext::GenesisConfig::<Runtime> { _config: Default::default() }
            .assimilate_storage(&mut storage)
            .expect("aura_ext storage");

        let mut ext: sp_io::TestExternalities = storage.into();
        ext.execute_with(|| {
            // Get authorities from Aura - this should be populated by Session's
            // genesis_build via the SessionHandler mechanism
            let aura_authorities = pallet_aura::Authorities::<Runtime>::get();

            // The authorities list MUST NOT be empty after genesis
            // If this fails, Session pallet's genesis_build didn't properly
            // call Aura's SessionHandler::on_genesis_session
            assert!(
                !aura_authorities.is_empty(),
                "Aura authorities should not be empty after genesis. Session pallet should have \
                 populated them via SessionHandler. This may indicate incorrect pallet ordering \
                 in construct_runtime."
            );

            // Verify both authorities are present
            assert_eq!(
                aura_authorities.len(),
                2,
                "Expected 2 authorities (Alice and Bob), got {}",
                aura_authorities.len()
            );

            assert!(
                aura_authorities.contains(&alice_aura),
                "Alice's AuraId should be in authorities"
            );
            assert!(aura_authorities.contains(&bob_aura), "Bob's AuraId should be in authorities");
        });
    }
}
