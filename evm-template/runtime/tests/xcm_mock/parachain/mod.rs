mod xcm_config;
use core::marker::PhantomData;

use evm_runtime_template::configs::xcm_config::SignedToAccountId20;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ContainsPair, Everything, Nothing},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use frame_system::EnsureRoot;
use sp_core::ConstU32;
use sp_runtime::traits::{Get, IdentityLookup};
use xcm::latest::prelude::*;
use xcm_builder::EnsureXcmOrigin;
pub use xcm_config::*;
use xcm_executor::XcmExecutor;
use xcm_simulator::mock_message_queue;

pub type AccountId = fp_account::AccountId20;
pub type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
    type AccountData = pallet_balances::AccountData<Balance>;
    type AccountId = AccountId;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1>;
}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_div(4), 0);
    pub const ReservedDmpWeight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_div(4), 0);
}

impl mock_message_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

pub type LocalOriginToLocation =
    SignedToAccountId20<RuntimeOrigin, AccountId, constants::RelayNetwork>;

pub struct TrustedLockerCase<T>(PhantomData<T>);
impl<T: Get<(Location, AssetFilter)>> ContainsPair<Location, Asset> for TrustedLockerCase<T> {
    fn contains(origin: &Location, asset: &Asset) -> bool {
        let (o, a) = T::get();
        a.matches(asset) && &o == origin
    }
}

parameter_types! {
    pub RelayTokenForRelay: (Location, AssetFilter) = (Parent.into(), Wild(AllOf { id: AssetId(Parent.into()), fun: WildFungible }));
}

pub type TrustedLockers = TrustedLockerCase<RelayTokenForRelay>;

impl pallet_xcm::Config for Runtime {
    type AdminOrigin = EnsureRoot<AccountId>;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type MaxLockers = ConstU32<8>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = location_converter::LocationConverter;
    type TrustedLockers = TrustedLockers;
    type UniversalLocation = constants::UniversalLocation;
    type Weigher = weigher::Weigher;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmReserveTransferFilter = Everything;
    type XcmRouter = XcmRouter;
    type XcmTeleportFilter = Nothing;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Balances: pallet_balances,
        MsgQueue: mock_message_queue,
        PolkadotXcm: pallet_xcm,
    }
);
