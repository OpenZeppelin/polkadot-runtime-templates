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
        AsEnsureOriginWithArg, ConstU32, ConstU64, Contains, EitherOfDiverse, InstanceFilter,
        TransformOrigin,
    },
    weights::{ConstantMultiplier, Weight},
    PalletId,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot, EnsureSigned,
};
pub use governance::origins::pallet_custom_origins;
use governance::{origins::Treasurer, TreasurySpender};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use polkadot_runtime_wrappers::{
    impl_openzeppelin_assets, impl_openzeppelin_consensus, impl_openzeppelin_governance,
    impl_openzeppelin_system, AssetsConfig, ConsensusConfig, GovernanceConfig, SystemConfig,
};
use scale_info::TypeInfo;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::{
    traits::{AccountIdLookup, BlakeTwo256, IdentityLookup},
    Perbill, Permill, RuntimeDebug,
};
use sp_version::RuntimeVersion;
use xcm::latest::{
    prelude::{AssetId, BodyId},
    InteriorLocation,
    Junction::PalletInstance,
};
#[cfg(not(feature = "runtime-benchmarks"))]
use xcm_builder::ProcessXcmMessage;
use xcm_config::{RelayLocation, XcmOriginToTransactDispatchOrigin};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmark::{OpenHrmpChannel, PayWithEnsure};
use crate::{
    constants::{
        currency::{deposit, CENTS, EXISTENTIAL_DEPOSIT, GRAND, MICROCENTS},
        AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MAX_BLOCK_LENGTH,
        NORMAL_DISPATCH_RATIO, SLOT_DURATION, VERSION,
    },
    types::{
        AccountId, AssetKind, Balance, Beneficiary, Block, BlockNumber,
        CollatorSelectionUpdateOrigin, ConsensusHook, Hash, Nonce,
        PriceForSiblingParachainDelivery, TreasuryPaymaster,
    },
    weights::{self, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    Aura, Balances, CollatorSelection, MessageQueue, OriginCaller, PalletInfo, ParachainSystem,
    Preimage, Runtime, RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason,
    RuntimeOrigin, RuntimeTask, Session, SessionKeys, System, Treasury, WeightToFee, XcmpQueue,
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
    type PreimageOrigin = EnsureRoot<AccountId>;
    type SS58Prefix = SS58Prefix;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type Version = Version;
}
impl ConsensusConfig for OpenZeppelinConfig {
    type CollatorSelectionUpdateOrigin = CollatorSelectionUpdateOrigin;
}
parameter_types! {
    pub const AssetDeposit: Balance = 10 * CENTS;
    pub const AssetAccountDeposit: Balance = deposit(1, 16);
    pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
impl AssetsConfig for OpenZeppelinConfig {
    type ApprovalDeposit = ApprovalDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type AssetDeposit = AssetDeposit;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
}
parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = 2 * GRAND;
    pub const ProposalBondMaximum: Balance = GRAND;
    pub const SpendPeriod: BlockNumber = 6 * DAYS;
    pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;
    pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    // The asset's interior location for the paying account. This is the Treasury
    // pallet instance (which sits at index 13).
    pub TreasuryInteriorLocation: InteriorLocation = PalletInstance(13).into();
}
impl GovernanceConfig for OpenZeppelinConfig {
    type PayoutSpendPeriod = PayoutSpendPeriod;
    type ProposalBond = ProposalBond;
    type ProposalBondMaximum = ProposalBondMaximum;
    type ProposalBondMinimum = ProposalBondMinimum;
    type VoteLockingPeriod = VoteLockingPeriod;
    type SpendPeriod = SpendPeriod;
    type TreasuryApproveOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
    type TreasuryPalletId = TreasuryPalletId;
    type TreasuryInteriorLocation = TreasuryInteriorLocation;
}
impl_openzeppelin_system!(OpenZeppelinConfig);
impl_openzeppelin_consensus!(OpenZeppelinConfig);
impl_openzeppelin_assets!(OpenZeppelinConfig);
impl_openzeppelin_governance!(OpenZeppelinConfig);

parameter_types! {
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
    pub const HeapSize: u32 = 64 * 1024;
    pub const MaxStale: u32 = 8;
}

impl pallet_message_queue::Config for Runtime {
    type HeapSize = HeapSize;
    type IdleMaxServiceWeight = MessageQueueServiceWeight;
    type MaxStale = MaxStale;
    #[cfg(feature = "runtime-benchmarks")]
    type MessageProcessor = pallet_message_queue::mock_helpers::NoopMessageProcessor<
        cumulus_primitives_core::AggregateMessageOrigin,
    >;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MessageProcessor = ProcessXcmMessage<
        AggregateMessageOrigin,
        xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
        RuntimeCall,
    >;
    // The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
    type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
    type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
    type RuntimeEvent = RuntimeEvent;
    type ServiceWeight = MessageQueueServiceWeight;
    type Size = u32;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_message_queue::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MaxInboundSuspended: u32 = 1000;
    /// The asset ID for the asset that we use to pay for message delivery fees.
    pub FeeAssetId: AssetId = AssetId(RelayLocation::get());
    /// The base fee for the message delivery fees. Kusama is based for the reference.
    pub const ToSiblingBaseDeliveryFee: u128 = CENTS.saturating_mul(3);
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type ChannelInfo = ParachainSystem;
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type MaxInboundSuspended = MaxInboundSuspended;
    /// Ensure that this value is not set to null (or NoPriceForMessageDelivery) to prevent spamming
    type PriceForSiblingDelivery = PriceForSiblingParachainDelivery;
    type RuntimeEvent = RuntimeEvent;
    type VersionWrapper = ();
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
    // Enqueue XCMP messages from siblings for later processing.
    type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
}
