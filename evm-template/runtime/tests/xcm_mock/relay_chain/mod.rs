mod xcm_config;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, Everything, Nothing, ProcessMessage, ProcessMessageError},
    weights::{Weight, WeightMeter},
};
use frame_system::EnsureRoot;
use polkadot_runtime_parachains::{
    configuration,
    inclusion::{AggregateMessageOrigin, UmpQueueId},
    origin, shared,
};
use sp_core::ConstU32;
use sp_runtime::{traits::IdentityLookup, AccountId32};
use xcm::latest::prelude::*;
use xcm_builder::{IsConcrete, SignedToAccountId32};
pub use xcm_config::*;
use xcm_executor::XcmExecutor;

pub type AccountId = AccountId32;
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

impl shared::Config for Runtime {
    type DisabledValidators = ();
}

impl configuration::Config for Runtime {
    type WeightInfo = configuration::TestWeightInfo;
}

pub type LocalOriginToLocation =
    SignedToAccountId32<RuntimeOrigin, AccountId, constants::RelayNetwork>;

impl pallet_xcm::Config for Runtime {
    type AdminOrigin = EnsureRoot<AccountId>;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = IsConcrete<constants::TokenLocation>;
    // Anyone can execute XCM messages locally...
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type MaxLockers = ConstU32<8>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = location_converter::LocationConverter;
    type TrustedLockers = ();
    type UniversalLocation = constants::UniversalLocation;
    type Weigher = weigher::Weigher;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmReserveTransferFilter = Everything;
    type XcmRouter = XcmRouter;
    type XcmTeleportFilter = Everything;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

impl origin::Config for Runtime {}

type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    /// Amount of weight that can be spent per block to service messages.
    pub MessageQueueServiceWeight: Weight = Weight::from_parts(1_000_000_000, 1_000_000);
    pub const MessageQueueHeapSize: u32 = 65_536;
    pub const MessageQueueMaxStale: u32 = 16;
}

/// Message processor to handle any messages that were enqueued into the `MessageQueue` pallet.
pub struct MessageProcessor;
impl ProcessMessage for MessageProcessor {
    type Origin = AggregateMessageOrigin;

    fn process_message(
        message: &[u8],
        origin: Self::Origin,
        meter: &mut WeightMeter,
        id: &mut [u8; 32],
    ) -> Result<bool, ProcessMessageError> {
        let para = match origin {
            AggregateMessageOrigin::Ump(UmpQueueId::Para(para)) => para,
        };
        xcm_builder::ProcessXcmMessage::<
			Junction,
			xcm_executor::XcmExecutor<XcmConfig>,
			RuntimeCall,
		>::process_message(message, Junction::Parachain(para.into()), meter, id)
    }
}

impl pallet_message_queue::Config for Runtime {
    type HeapSize = MessageQueueHeapSize;
    type IdleMaxServiceWeight = ();
    type MaxStale = MessageQueueMaxStale;
    type MessageProcessor = MessageProcessor;
    type QueueChangeHandler = ();
    type QueuePausedQuery = ();
    type RuntimeEvent = RuntimeEvent;
    type ServiceWeight = MessageQueueServiceWeight;
    type Size = u32;
    type WeightInfo = ();
}

construct_runtime!(
    pub enum Runtime
    {
        System: frame_system,
        Balances: pallet_balances,
        ParasOrigin: origin,
        XcmPallet: pallet_xcm,
        MessageQueue: pallet_message_queue,
    }
);
