#![cfg(test)]

use crate as pallet;
use frame_support::{construct_runtime, derive_impl, parameter_types};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

pub type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        ZkVerifier: pallet,
    }
);

parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(
            frame_support::weights::Weight::from_parts(1024, u64::MAX),
        );
    pub static ExistentialDeposit: u64 = 1;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RangeVerifier = zk_elgamal_verifier::BulletproofRangeVerifier;
}

pub fn new_test_ext() -> TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("valid default genesis storage");
    TestExternalities::from(storage)
}

#[test]
fn builds_runtime_successfully() {
    new_test_ext().execute_with(|| {
        assert_eq!(1 + 1, 2);
    });
}
