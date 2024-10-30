//! Relay chain runtime mock.

use frame_support::{
    construct_runtime, parameter_types,
    traits::{Everything, Nothing, ProcessMessage, ProcessMessageError},
    weights::{Weight, WeightMeter},
};
use frame_system::pallet_prelude::BlockNumberFor;
use polkadot_parachain_primitives::primitives::Id as ParaId;
use polkadot_runtime_parachains::{
    configuration, dmp, hrmp,
    inclusion::{AggregateMessageOrigin, UmpQueueId},
    origin, paras, shared,
};
use sp_core::H256;
use sp_runtime::{
    traits::{ConstU32, IdentityLookup},
    transaction_validity::TransactionPriority,
    AccountId32, Permill,
};
use xcm::latest::prelude::*;
use xcm_builder::{
    Account32Hash, AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
    AllowTopLevelPaidExecutionFrom, ChildParachainAsNative, ChildParachainConvertsVia,
    ChildSystemParachainAsSuperuser, FixedRateOfFungible, FixedWeightBounds,
    FungibleAdapter as XcmCurrencyAdapter, IsConcrete, ProcessXcmMessage,
    SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
    WithComputedOrigin,
};
use xcm_executor::{Config, XcmExecutor};
pub type AccountId = AccountId32;
pub type Balance = u128;
pub type BlockNumber = BlockNumberFor<Runtime>;

parameter_types! {
    pub const BlockHashCount: u32 = 250;
}

impl frame_system::Config for Runtime {
    type AccountData = pallet_balances::AccountData<Balance>;
    type AccountId = AccountId;
    type BaseCallFilter = Everything;
    type Block = Block;
    type BlockHashCount = BlockHashCount;
    type BlockLength = ();
    type BlockWeights = ();
    type DbWeight = ();
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type Lookup = IdentityLookup<Self::AccountId>;
    type MaxConsumers = ConstU32<16>;
    type Nonce = u64;
    type OnKilledAccount = ();
    type OnNewAccount = ();
    type OnSetCode = ();
    type PalletInfo = PalletInfo;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeTask = RuntimeTask;
    type SS58Prefix = ();
    type SystemWeightInfo = ();
    type Version = ();
}

parameter_types! {
    pub ExistentialDeposit: Balance = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type RuntimeEvent = RuntimeEvent;
    type RuntimeFreezeReason = ();
    type RuntimeHoldReason = ();
    type WeightInfo = ();
}

impl pallet_utility::Config for Runtime {
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl shared::Config for Runtime {
    type DisabledValidators = ();
}

impl configuration::Config for Runtime {
    type WeightInfo = configuration::TestWeightInfo;
}

parameter_types! {
    pub KsmLocation: Location = Here.into();
    pub const KusamaNetwork: NetworkId = NetworkId::Kusama;
    pub const AnyNetwork: Option<NetworkId> = None;
    pub UniversalLocation: InteriorLocation = Here;
}

pub type SovereignAccountOf = (
    ChildParachainConvertsVia<ParaId, AccountId>,
    AccountId32Aliases<KusamaNetwork, AccountId>,
    // Not enabled in the relay per se, but we enable it to test
    // the transact_through_signed extrinsic
    Account32Hash<KusamaNetwork, AccountId>,
);

pub type LocalAssetTransactor =
    XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, SovereignAccountOf, AccountId, ()>;

type LocalOriginConverter = (
    SovereignSignedViaLocation<SovereignAccountOf, RuntimeOrigin>,
    ChildParachainAsNative<origin::Origin, RuntimeOrigin>,
    SignedAccountId32AsNative<KusamaNetwork, RuntimeOrigin>,
    ChildSystemParachainAsSuperuser<ParaId, RuntimeOrigin>,
);

parameter_types! {
    pub const BaseXcmWeight: Weight = Weight::from_parts(1000u64, 1000u64);
    pub KsmPerSecond: (AssetId, u128, u128) = (AssetId(KsmLocation::get()), 1, 1);
    pub const MaxInstructions: u32 = 100;
    pub const MaxAssetsIntoHolding: u32 = 64;
    pub MatcherLocation: Location = Location::here();
}

pub type XcmRouter = super::RelayChainXcmRouter;

pub type XcmBarrier = (
    // Weight that is paid for may be consumed.
    TakeWeightCredit,
    // Expected responses are OK.
    AllowKnownQueryResponses<XcmPallet>,
    WithComputedOrigin<
        (
            // If the message is one that immediately attempts to pay for execution, then allow it.
            AllowTopLevelPaidExecutionFrom<Everything>,
            // Subscriptions for version tracking are OK.
            AllowSubscriptionsFrom<Everything>,
        ),
        UniversalLocation,
        ConstU32<8>,
    >,
);

parameter_types! {
    pub Kusama: AssetFilter = Wild(AllOf { fun: WildFungible, id: AssetId(KsmLocation::get()) });
    pub Statemine: Location = Parachain(4).into();
    pub KusamaForStatemine: (AssetFilter, Location) = (Kusama::get(), Statemine::get());
}

pub type TrustedTeleporters = xcm_builder::Case<KusamaForStatemine>;

pub struct XcmConfig;
impl Config for XcmConfig {
    type Aliasers = Nothing;
    type AssetClaims = XcmPallet;
    type AssetExchanger = ();
    type AssetLocker = ();
    type AssetTransactor = LocalAssetTransactor;
    type AssetTrap = XcmPallet;
    type Barrier = XcmBarrier;
    type CallDispatcher = RuntimeCall;
    type FeeManager = ();
    type IsReserve = ();
    type IsTeleporter = TrustedTeleporters;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type MessageExporter = ();
    type OriginConverter = LocalOriginConverter;
    type PalletInstancesInfo = ();
    type ResponseHandler = XcmPallet;
    type RuntimeCall = RuntimeCall;
    type SafeCallFilter = Everything;
    type SubscriptionService = XcmPallet;
    type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
    type TransactionalProcessor = ();
    type UniversalAliases = Nothing;
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type XcmSender = XcmRouter;
}

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, KusamaNetwork>;

impl pallet_xcm::Config for Runtime {
    type AdminOrigin = frame_system::EnsureRoot<AccountId>;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = ();
    // Anyone can execute XCM messages locally...
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type MaxLockers = ConstU32<8>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = ();
    type TrustedLockers = ();
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmReserveTransferFilter = Everything;
    type XcmRouter = XcmRouter;
    type XcmTeleportFilter = Everything;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

parameter_types! {
    pub const FirstMessageFactorPercent: u64 = 100;
}

parameter_types! {
    pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

/// A very dumb implementation of `EstimateNextSessionRotation`. At the moment of writing, this
/// is more to satisfy type requirements rather than to test anything.
pub struct TestNextSessionRotation;

impl frame_support::traits::EstimateNextSessionRotation<u32> for TestNextSessionRotation {
    fn average_session_length() -> u32 {
        10
    }

    fn estimate_current_session_progress(_now: u32) -> (Option<Permill>, Weight) {
        (None, Weight::zero())
    }

    fn estimate_next_session_rotation(_now: u32) -> (Option<u32>, Weight) {
        (None, Weight::zero())
    }
}

impl paras::Config for Runtime {
    type AssignCoretime = ();
    type NextSessionRotation = TestNextSessionRotation;
    type OnNewHead = ();
    type QueueFootprinter = ();
    type RuntimeEvent = RuntimeEvent;
    type UnsignedPriority = ParasUnsignedPriority;
    type WeightInfo = paras::TestWeightInfo;
}

impl dmp::Config for Runtime {}

impl hrmp::Config for Runtime {
    type ChannelManager = frame_system::EnsureRoot<AccountId>;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type WeightInfo = TestHrmpWeightInfo;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

impl origin::Config for Runtime {}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlockU32<Runtime>;

parameter_types! {
    pub MessageQueueServiceWeight: Weight = Weight::from_parts(1_000_000_000, 1_000_000);
    pub const MessageQueueHeapSize: u32 = 65_536;
    pub const MessageQueueMaxStale: u32 = 16;
}

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
        ProcessXcmMessage::<Junction, XcmExecutor<XcmConfig>, RuntimeCall>::process_message(
            message,
            Junction::Parachain(para.into()),
            meter,
            id,
        )
    }
}

impl pallet_message_queue::Config for Runtime {
    type HeapSize = MessageQueueHeapSize;
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
    pub enum Runtime	{
        System: frame_system,
        Balances: pallet_balances,
        ParasOrigin: origin,
        MessageQueue: pallet_message_queue,
        XcmPallet: pallet_xcm,
        Utility: pallet_utility,
        Hrmp: hrmp,
        Dmp: dmp,
        Paras: paras,
        Configuration: configuration,
    }
);

pub(crate) fn relay_events() -> Vec<RuntimeEvent> {
    System::events().into_iter().map(|r| r.event).filter_map(|e| Some(e)).collect::<Vec<_>>()
}

use frame_support::traits::{OnFinalize, OnInitialize};
pub(crate) fn relay_roll_to(n: BlockNumber) {
    while System::block_number() < n {
        XcmPallet::on_finalize(System::block_number());
        Balances::on_finalize(System::block_number());
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        Balances::on_initialize(System::block_number());
        XcmPallet::on_initialize(System::block_number());
    }
}

/// A weight info that is only suitable for testing.
pub struct TestHrmpWeightInfo;

impl hrmp::WeightInfo for TestHrmpWeightInfo {
    fn hrmp_accept_open_channel() -> Weight {
        Weight::from_parts(1, 0)
    }

    fn force_clean_hrmp(_: u32, _: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn force_process_hrmp_close(_: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn force_process_hrmp_open(_: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn hrmp_cancel_open_request(_: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn hrmp_close_channel() -> Weight {
        Weight::from_parts(1, 0)
    }

    fn hrmp_init_open_channel() -> Weight {
        Weight::from_parts(1, 0)
    }

    fn clean_open_channel_requests(_: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn force_open_hrmp_channel(_: u32) -> Weight {
        Weight::from_parts(1, 0)
    }

    fn establish_system_channel() -> Weight {
        Weight::from_parts(1, 0)
    }

    fn poke_channel_deposits() -> Weight {
        Weight::from_parts(1, 0)
    }
}
