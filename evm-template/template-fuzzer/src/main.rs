use std::{
    iter,
    time::{Duration, Instant},
};

use clap::Args;
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

mod random;
mod assets;

type ExtrinsicData = Vec<(/* lapse */ u8, /* origin */ u8, RuntimeCall)>;

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
        session: SessionConfig { keys: session_keys },
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

fn process_input(
    accounts: &[AccountId], 
    genesis: &Storage, 
    data: &mut [u8], 
    generate_extrinsics: impl Fn(&mut [u8]) -> ExtrinsicData,
    call_filter: impl Fn(&RuntimeCall, usize) -> bool,
) {
    // We build the list of extrinsics we will execute
    let extrinsics: ExtrinsicData = generate_extrinsics(data);
        
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
            if !call_filter(&extrinsic, origin_no) {
                continue;
            }
            if lapse > 0 {
                finalize_block(elapsed);

                block += u32::from(lapse) * 393; // 393 * 256 = 100608 which nearly corresponds to a week
                weight = 0.into();
                elapsed = Duration::ZERO;

                initialize_block(block);
            }

            weight.saturating_accrue(extrinsic.get_dispatch_info().weight);
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

        finalize_block(elapsed);

        check_invariants(block, initial_total_issuance);
    });
}

fn initialize_block(block: u32) {
    println!("\ninitializing block {}", block);

    let current_timestamp = u64::from(block) * SLOT_DURATION;

    let prev_header = match block {
        1 => None,
        _ => Some(Executive::finalize_block()),
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
    Executive::apply_extrinsic(UncheckedExtrinsic::new_unsigned(RuntimeCall::Timestamp(
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

    Executive::apply_extrinsic(UncheckedExtrinsic::new_unsigned(RuntimeCall::ParachainSystem(
        parachain_validation_data,
    )))
    .unwrap()
    .unwrap();

    // Calls that need to be called before each block starts (init_calls) go here
}

fn finalize_block(elapsed: Duration) {
    println!("\n  time spent: {elapsed:?}");
    assert!(elapsed.as_secs() <= 2, "block execution took too much time");

    println!("  finalizing block");
    Executive::finalize_block();
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
            Balances::locks(account).iter().map(|l| l.amount).max().unwrap_or_default();
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
    // let args = cli::Args::parse();
    let accounts: Vec<AccountId> = (0..5).map(|i| [i; 32].into()).collect();
    let genesis = generate_genesis(&accounts);

    // if args.random {
        // (random::generate_extrinsic_stream, random::recursive_call_filter)
    // } else if args.assets {
        
    // } else {
    //     println!("Mode was not specified. Halting");
    //     return;
    // };

    ziggy::fuzz!(|data: &[u8]| {
        let mut data = data.to_vec();
        process_input(&accounts, &genesis, &mut data, assets::generate_extrinsics_stream, |_, _| true);
    });
}