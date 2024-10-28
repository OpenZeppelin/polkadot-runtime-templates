pub mod asset_config;
pub mod governance;
pub mod xcm_config;

#[cfg(feature = "async-backing")]
use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
#[cfg(not(feature = "async-backing"))]
use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
    derive_impl,
    dispatch::DispatchClass,
    parameter_types,
    traits::{
        ConstU32, ConstU64, Contains, EitherOf, EitherOfDiverse, Everything, InstanceFilter,
        Nothing, TransformOrigin,
    },
    weights::Weight,
    PalletId,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
pub use governance::origins::pallet_custom_origins;
use governance::{origins::Treasurer, tracks, Spender, WhitelistedCaller};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::{impls::ToAuthor, BlockHashCount};
use polkadot_runtime_wrappers::{
    impl_openzeppelin_consensus, impl_openzeppelin_governance, impl_openzeppelin_system,
    impl_openzeppelin_xcm, ConsensusConfig, GovernanceConfig, SystemConfig, XcmConfig,
};
use scale_info::TypeInfo;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::{
    traits::{AccountIdLookup, BlakeTwo256, IdentityLookup},
    Perbill, RuntimeDebug,
};
use sp_version::RuntimeVersion;
use xcm::latest::{prelude::*, InteriorLocation, Junction::PalletInstance};
#[cfg(not(feature = "runtime-benchmarks"))]
use xcm_builder::ProcessXcmMessage;
use xcm_builder::{
    AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom,
    DenyReserveTransferToRelayChain, DenyThenTry, EnsureXcmOrigin, FixedWeightBounds,
    FrameTransactionalProcessor, NativeAsset, TakeWeightCredit, TrailingSetTopicAsId,
    UsingComponents, WithComputedOrigin, WithUniqueTopic,
};
use xcm_config::*;
use xcm_executor::XcmExecutor;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmark::{OpenHrmpChannel, PayWithEnsure};
use crate::{
    constants::{
        currency::{deposit, CENTS, EXISTENTIAL_DEPOSIT},
        AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MAX_BLOCK_LENGTH,
        NORMAL_DISPATCH_RATIO, VERSION,
    },
    types::{
        AccountId, AssetKind, Balance, Beneficiary, Block, BlockNumber,
        CollatorSelectionUpdateOrigin, ConsensusHook, Hash, Nonce,
        PriceForSiblingParachainDelivery, TreasuryPaymaster,
    },
    weights::{self, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    AllPalletsWithSystem, Aura, Balances, CollatorSelection, MessageQueue, OriginCaller,
    PalletInfo, ParachainInfo, ParachainSystem, PolkadotXcm, Preimage, Referenda, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
    Scheduler, Session, SessionKeys, System, Treasury, WeightToFee, XcmpQueue,
};

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
    // generic substrate prefix. For more info, see: [Polkadot Accounts In-Depth](https://wiki.polkadot.network/docs/learn-account-advanced#:~:text=The%20address%20format%20used%20in,belonging%20to%20a%20specific%20network)
    pub const SS58Prefix: u16 = 42;
    pub const Version: RuntimeVersion = VERSION;
}
/// OpenZeppelin configuration
pub struct OpenZeppelinConfig;
impl SystemConfig for OpenZeppelinConfig {
    type AccountId = AccountId;
    type ExistentialDeposit = ExistentialDeposit;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type PreimageOrigin = EnsureRoot<AccountId>;
    type SS58Prefix = SS58Prefix;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type Version = Version;
}
impl ConsensusConfig for OpenZeppelinConfig {
    type CollatorSelectionUpdateOrigin = CollatorSelectionUpdateOrigin;
}
parameter_types! {
    pub const AlarmInterval: BlockNumber = 1;
    pub const SubmissionDeposit: Balance = 3 * CENTS;
    pub const UndecidingTimeout: BlockNumber = 14 * DAYS;
    pub const SpendPeriod: BlockNumber = 6 * DAYS;
    pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;
    pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub TreasuryAccount: AccountId = Treasury::account_id();
    // The asset's interior location for the paying account. This is the Treasury
    // pallet instance (which sits at index 13).
    pub TreasuryInteriorLocation: InteriorLocation = PalletInstance(13).into();
}
impl GovernanceConfig for OpenZeppelinConfig {
    type ConvictionVoteLockingPeriod = VoteLockingPeriod;
    type DispatchWhitelistedOrigin = EitherOf<EnsureRoot<AccountId>, WhitelistedCaller>;
    type ReferendaAlarmInterval = AlarmInterval;
    type ReferendaCancelOrigin = EnsureRoot<AccountId>;
    type ReferendaKillOrigin = EnsureRoot<AccountId>;
    type ReferendaSlash = Treasury;
    type ReferendaSubmissionDeposit = SubmissionDeposit;
    type ReferendaSubmitOrigin = EnsureSigned<AccountId>;
    type ReferendaUndecidingTimeout = UndecidingTimeout;
    type TreasuryInteriorLocation = TreasuryInteriorLocation;
    type TreasuryPalletId = TreasuryPalletId;
    type TreasuryPayoutSpendPeriod = PayoutSpendPeriod;
    type TreasuryRejectOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
    type TreasurySpendOrigin = TreasurySpender;
    type TreasurySpendPeriod = SpendPeriod;
    type WhitelistOrigin = EnsureRoot<AccountId>;
}
parameter_types! {
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
    pub const HeapSize: u32 = 64 * 1024;
    pub const MaxStale: u32 = 8;
    pub const MaxInboundSuspended: u32 = 1000;
}
impl XcmConfig for OpenZeppelinConfig {
    type AssetTransactors = AssetTransactors;
    type FeeManager = FeeManager;
    type LocalOriginToLocation = LocalOriginToLocation;
    type LocationToAccountId = LocationToAccountId;
    type MessageQueueHeapSize = HeapSize;
    type MessageQueueMaxStale = MaxStale;
    type MessageQueueServiceWeight = MessageQueueServiceWeight;
    type Reserves = NativeAsset;
    type Trader =
        UsingComponents<WeightToFee, RelayLocation, AccountId, Balances, ToAuthor<Runtime>>;
    type XcmAdminOrigin = EnsureRoot<AccountId>;
    type XcmOriginToTransactDispatchOrigin = XcmOriginToTransactDispatchOrigin;
    type XcmpQueueControllerOrigin = EnsureRoot<AccountId>;
    type XcmpQueueMaxInboundSuspended = MaxInboundSuspended;
}
impl_openzeppelin_system!(OpenZeppelinConfig);
impl_openzeppelin_consensus!(OpenZeppelinConfig);
impl_openzeppelin_governance!(OpenZeppelinConfig);
impl_openzeppelin_xcm!(OpenZeppelinConfig);
