use std::{
    iter,
    time::{Duration, Instant},
};

// Local Imports
use evm_runtime_template::{
    constants::SLOT_DURATION, AccountId, AllPalletsWithSystem, Balance, Balances, EVMChainIdConfig,
    Executive, Runtime, RuntimeCall, RuntimeOrigin, SudoConfig, UncheckedExtrinsic,
};
#[cfg(not(feature = "tanssi"))]
use frame_support::traits::Get;
use frame_support::{
    dispatch::GetDispatchInfo,
    traits::{IntegrityTest, TryState, TryStateSelect},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use frame_system::Account;
use pallet_balances::{Holds, TotalIssuance};
use parity_scale_codec::{DecodeLimit, Encode};
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_runtime::{
    testing::H256,
    traits::{Dispatchable, Header},
    Digest, DigestItem, Storage,
};
use sp_state_machine::BasicExternalities;

fn generate_genesis(accounts: &[AccountId]) -> Storage {
    use evm_runtime_template::{BalancesConfig, RuntimeGenesisConfig};
    #[cfg(not(feature = "tanssi"))]
    use evm_runtime_template::{CollatorSelectionConfig, SessionConfig, SessionKeys};
    #[cfg(not(feature = "tanssi"))]
    use sp_consensus_aura::sr25519::AuthorityId as AuraId;
    #[cfg(not(feature = "tanssi"))]
    use sp_runtime::app_crypto::ByteArray;
    use sp_runtime::BuildStorage;

    // Configure endowed accounts with initial balance of 1 << 60.
    let balances = accounts.iter().cloned().map(|k| (k, 1 << 60)).collect();
    #[cfg(not(feature = "tanssi"))]
    let invulnerables: Vec<AccountId> = vec![[0; 32].into()];
    #[cfg(not(feature = "tanssi"))]
    let session_keys = vec![(
        [0; 32].into(),
        [0; 32].into(),
        SessionKeys { aura: AuraId::from_slice(&[0; 32]).unwrap() },
    )];
    let root: AccountId = [0; 32].into();

    RuntimeGenesisConfig {
        system: Default::default(),
        balances: BalancesConfig { balances },
        #[cfg(not(feature = "tanssi"))]
        session: SessionConfig { keys: session_keys, non_authority_keys: vec![] },
        #[cfg(not(feature = "tanssi"))]
        collator_selection: CollatorSelectionConfig {
            invulnerables,
            candidacy_bond: 1 << 57,
            desired_candidates: 1,
        },
        parachain_info: Default::default(),
        parachain_system: Default::default(),
        polkadot_xcm: Default::default(),
        assets: Default::default(),
        transaction_payment: Default::default(),
        sudo: SudoConfig { key: Some(root) },
        treasury: Default::default(),
        base_fee: Default::default(), // TODO: reconsider default value
        evm_chain_id: EVMChainIdConfig {
            chain_id: 1000, // TODO: select a good value
            ..Default::default()
        },
        evm: Default::default(),
        ethereum: Default::default(),
        ..Default::default()
    }
    .build_storage()
    .unwrap()
}

fn process_input(accounts: &[AccountId], genesis: &Storage, data: &[u8]) {
    let mut data = data;
    // We build the list of extrinsics we will execute
    let extrinsics: Vec<(/* lapse */ u8, /* origin */ u8, RuntimeCall)> =
        iter::from_fn(|| DecodeLimit::decode_with_depth_limit(64, &mut data).ok())
            .filter(|(_, _, x)| !matches!(x, RuntimeCall::System(_)))
            .collect();
    if extrinsics.is_empty() {
        return;
    }

    let mut block: u32 = 1;
    let mut weight: Weight = 0.into();
    let mut elapsed: Duration = Duration::ZERO;

    BasicExternalities::execute_with_storage(&mut genesis.clone(), || {
        let initial_total_issuance = TotalIssuance::<Runtime>::get();

        initialize_block(block);

        for (lapse, origin, extrinsic) in extrinsics {
            let origin_no = origin as usize % accounts.len();
            if !recursive_call_filter(&extrinsic, origin_no) {
                continue;
            }
            if lapse > 0 {
                println!("\n  time spent: {elapsed:?}");
                assert!(elapsed.as_secs() <= 2, "block execution took too much time");

                block += u32::from(lapse) * 393; // 393 * 256 = 100608 which nearly corresponds to a week
                weight = 0.into();
                elapsed = Duration::ZERO;

                initialize_block(block);
            }

            weight.saturating_accrue(extrinsic.get_dispatch_info().call_weight);
            if weight.ref_time() >= 2 * WEIGHT_REF_TIME_PER_SECOND {
                println!("Extrinsic would exhaust block weight, skipping");
                continue;
            }

            let origin = accounts[origin_no];

            println!("\n    origin:     {origin:?}");
            println!("    call:       {extrinsic:?}");

            let now = Instant::now(); // We get the current time for timing purposes.

            #[allow(unused_variables)]
            let res = extrinsic.dispatch(RuntimeOrigin::signed(origin));
            elapsed += now.elapsed();

            println!("    result:     {res:?}");
        }

        Executive::finalize_block();

        check_invariants(block, initial_total_issuance);
    });
}

fn initialize_block(block: u32) {
    println!("\ninitializing block {}", block);

    let current_timestamp = u64::from(block) * SLOT_DURATION * 2;

    let prev_header = match block {
        1 => None,
        _ => {
            println!("  finalizing block");
            Some(Executive::finalize_block())
        }
    };

    let parent_header = &Header::new(
        block,
        H256::default(),
        H256::default(),
        prev_header.clone().map(|x| x.hash()).unwrap_or_default(),
        Digest {
            logs: vec![DigestItem::PreRuntime(
                AURA_ENGINE_ID,
                Slot::from(u64::from(block)).encode(),
            )],
        },
    );

    Executive::initialize_block(parent_header);

    // We apply the timestamp extrinsic for the current block.
    Executive::apply_extrinsic(UncheckedExtrinsic::new_bare(RuntimeCall::Timestamp(
        pallet_timestamp::Call::set { now: current_timestamp },
    )))
    .unwrap()
    .unwrap();

    let parachain_validation_data = {
        use cumulus_primitives_core::{relay_chain::HeadData, PersistedValidationData};
        use cumulus_primitives_parachain_inherent::ParachainInherentData;
        use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;

        let parent_head = HeadData(prev_header.clone().unwrap_or(parent_header.clone()).encode());
        let sproof_builder = RelayStateSproofBuilder {
            para_id: 100.into(),
            current_slot: Slot::from(2 * current_timestamp / SLOT_DURATION),
            included_para_head: Some(parent_head.clone()),
            ..Default::default()
        };

        let (relay_parent_storage_root, relay_chain_state) =
            sproof_builder.into_state_root_and_proof();
        let data = ParachainInherentData {
            validation_data: PersistedValidationData {
                parent_head,
                relay_parent_number: block,
                relay_parent_storage_root,
                max_pov_size: 1000,
            },
            relay_chain_state,
            downward_messages: Default::default(),
            horizontal_messages: Default::default(),
        };
        cumulus_pallet_parachain_system::Call::set_validation_data { data }
    };

    Executive::apply_extrinsic(UncheckedExtrinsic::new_bare(RuntimeCall::ParachainSystem(
        parachain_validation_data,
    )))
    .unwrap()
    .unwrap();

    // Calls that need to be called before each block starts (init_calls) go here
}

fn recursive_call_filter(call: &RuntimeCall, origin: usize) -> bool {
    match call {
        //recursion
        RuntimeCall::Sudo(
            pallet_sudo::Call::sudo { call }
            | pallet_sudo::Call::sudo_unchecked_weight { call, weight: _ },
        ) if origin == 0 => recursive_call_filter(call, origin),
        RuntimeCall::Utility(
            pallet_utility::Call::with_weight { call, weight: _ }
            | pallet_utility::Call::dispatch_as { as_origin: _, call }
            | pallet_utility::Call::as_derivative { index: _, call },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Utility(
            pallet_utility::Call::force_batch { calls }
            | pallet_utility::Call::batch { calls }
            | pallet_utility::Call::batch_all { calls },
        ) => calls.iter().map(|call| recursive_call_filter(call, origin)).all(|e| e),
        RuntimeCall::Scheduler(
            pallet_scheduler::Call::schedule_named_after {
                id: _,
                after: _,
                maybe_periodic: _,
                priority: _,
                call,
            }
            | pallet_scheduler::Call::schedule { when: _, maybe_periodic: _, priority: _, call }
            | pallet_scheduler::Call::schedule_named {
                when: _,
                id: _,
                maybe_periodic: _,
                priority: _,
                call,
            }
            | pallet_scheduler::Call::schedule_after {
                after: _,
                maybe_periodic: _,
                priority: _,
                call,
            },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Multisig(
            pallet_multisig::Call::as_multi_threshold_1 { other_signatories: _, call }
            | pallet_multisig::Call::as_multi {
                threshold: _,
                other_signatories: _,
                maybe_timepoint: _,
                call,
                max_weight: _,
            },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Whitelist(
            pallet_whitelist::Call::dispatch_whitelisted_call_with_preimage { call },
        ) => recursive_call_filter(call, origin),

        // restrictions
        RuntimeCall::Sudo(_) if origin != 0 => false,
        RuntimeCall::System(
            frame_system::Call::set_code { .. } | frame_system::Call::kill_prefix { .. },
        ) => false,
        #[cfg(not(feature = "tanssi"))]
        RuntimeCall::CollatorSelection(
            pallet_collator_selection::Call::set_desired_candidates { max },
        ) =>
            *max < <<Runtime as pallet_collator_selection::Config>::MaxCandidates as Get<u32>>::get(
            ),
        RuntimeCall::Balances(pallet_balances::Call::force_adjust_total_issuance { .. }) => false,

        _ => true,
    }
}

fn check_invariants(block: u32, initial_total_issuance: Balance) {
    let mut counted_free = 0;
    let mut counted_reserved = 0;
    for (account, info) in Account::<Runtime>::iter() {
        let consumers = info.consumers;
        let providers = info.providers;
        assert!(!(consumers > 0 && providers == 0), "Invalid c/p state");
        counted_free += info.data.free;
        counted_reserved += info.data.reserved;
        let max_lock: Balance =
            Balances::locks(&account).iter().map(|l| l.amount).max().unwrap_or_default();
        assert_eq!(max_lock, info.data.frozen, "Max lock should be equal to frozen balance");
        let sum_holds: Balance = Holds::<Runtime>::get(account).iter().map(|l| l.amount).sum();
        assert!(
            sum_holds <= info.data.reserved,
            "Sum of all holds ({sum_holds}) should be less than or equal to reserved balance {}",
            info.data.reserved
        );
    }
    let total_issuance = TotalIssuance::<Runtime>::get();
    let counted_issuance = counted_free + counted_reserved;
    assert_eq!(total_issuance, counted_issuance);
    assert!(total_issuance <= initial_total_issuance);
    // We run all developer-defined integrity tests
    AllPalletsWithSystem::integrity_test();
    AllPalletsWithSystem::try_state(block, TryStateSelect::All).unwrap();
}

fn main() {
    let accounts: Vec<AccountId> = (0..5).map(|i| [i; 32].into()).collect();
    let genesis = generate_genesis(&accounts);

    ziggy::fuzz!(|data: &[u8]| {
        process_input(&accounts, &genesis, data);
    });
}
