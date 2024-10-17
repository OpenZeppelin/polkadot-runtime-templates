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
    pub const Version: RuntimeVersion = VERSION;

    // This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
    //  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
    // `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
    // the lazy contract deletion.
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(MAX_BLOCK_LENGTH, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
    // generic substrate prefix. For more info, see: [Polkadot Accounts In-Depth](https://wiki.polkadot.network/docs/learn-account-advanced#:~:text=The%20address%20format%20used%20in,belonging%20to%20a%20specific%20network)
    pub const SS58Prefix: u16 = 42;
}

pub struct NormalFilter;
impl Contains<RuntimeCall> for NormalFilter {
    fn contains(c: &RuntimeCall) -> bool {
        match c {
            // We filter anonymous proxy as they make "reserve" inconsistent
            // See: https://github.com/paritytech/polkadot-sdk/blob/v1.9.0-rc2/substrate/frame/proxy/src/lib.rs#L260
            RuntimeCall::Proxy(method) => !matches!(
                method,
                pallet_proxy::Call::create_pure { .. }
                    | pallet_proxy::Call::kill_pure { .. }
                    | pallet_proxy::Call::remove_proxies { .. }
            ),
            _ => true,
        }
    }
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`ParaChainDefaultConfig`](`struct@frame_system::config_preludes::ParaChainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = NormalFilter;
    /// The block type.
    type Block = Block;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The lookup mechanism to get account ID from whatever is passed in
    /// dispatchers.
    type Lookup = AccountIdLookup<AccountId, ()>;
    /// The maximum number of consumers allowed on a single account.
    type MaxConsumers = ConstU32<16>;
    /// The index type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The action to take on a Runtime Upgrade
    type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
    /// Converts a module to an index of this module in the runtime.
    type PalletInfo = PalletInfo;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    /// Runtime version.
    type Version = Version;
}

parameter_types! {
    pub MaximumSchedulerWeight: frame_support::weights::Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
    pub const MaxScheduledRuntimeCallsPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Runtime {
    type MaxScheduledPerBlock = MaxScheduledRuntimeCallsPerBlock;
    type MaximumWeight = MaximumSchedulerWeight;
    type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
    type PalletsOrigin = OriginCaller;
    type Preimages = Preimage;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
}

parameter_types! {
    pub const PreimageBaseDeposit: Balance = deposit(2, 64);
    pub const PreimageByteDeposit: Balance = deposit(0, 1);
    pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type Consideration = frame_support::traits::fungible::HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        frame_support::traits::LinearStoragePrice<
            PreimageBaseDeposit,
            PreimageByteDeposit,
            Balance,
        >,
    >;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    #[cfg(feature = "experimental")]
    type MinimumPeriod = ConstU64<0>;
    #[cfg(not(feature = "experimental"))]
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type EventHandler = (CollatorSelection,);
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
}

parameter_types! {
    pub const MaxProxies: u32 = 32;
    pub const MaxPending: u32 = 32;
    pub const ProxyDepositBase: Balance = deposit(1, 40);
    pub const AnnouncementDepositBase: Balance = deposit(1, 48);
    pub const ProxyDepositFactor: Balance = deposit(0, 33);
    pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
}

/// The type used to represent the kinds of proxying allowed.
/// If you are adding new pallets, consider adding new ProxyType variant
#[derive(
    Copy,
    Clone,
    Decode,
    Default,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    RuntimeDebug,
    TypeInfo,
)]
pub enum ProxyType {
    /// Allows to proxy all calls
    #[default]
    Any,
    /// Allows all non-transfer calls
    NonTransfer,
    /// Allows to finish the proxy
    CancelProxy,
    /// Allows to operate with collators list (invulnerables, candidates, etc.)
    Collator,
}

impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => !matches!(c, RuntimeCall::Balances { .. }),
            ProxyType::CancelProxy => matches!(
                c,
                RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })
                    | RuntimeCall::Multisig { .. }
            ),
            ProxyType::Collator => {
                matches!(c, RuntimeCall::CollatorSelection { .. } | RuntimeCall::Multisig { .. })
            }
        }
    }
}

impl pallet_proxy::Config for Runtime {
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
    type CallHasher = BlakeTwo256;
    type Currency = Balances;
    type MaxPending = MaxPending;
    type MaxProxies = MaxProxies;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type ProxyType = ProxyType;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
    pub const MaxFreezes: u32 = 0;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type FreezeIdentifier = ();
    type MaxFreezes = MaxFreezes;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RuntimeHoldReason = RuntimeHoldReason;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}

parameter_types! {
    pub const AssetDeposit: Balance = 10 * CENTS;
    pub const AssetAccountDeposit: Balance = deposit(1, 16);
    pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
    pub const StringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = deposit(1, 68);
    pub const MetadataDepositPerByte: Balance = deposit(0, 1);
    pub const RemoveItemsLimit: u32 = 1000;
}

impl pallet_assets::Config for Runtime {
    type ApprovalDeposit = ApprovalDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type AssetDeposit = AssetDeposit;
    type AssetId = u32;
    type AssetIdParameter = parity_scale_codec::Compact<u32>;
    type Balance = Balance;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
    type CallbackHandle = ();
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Currency = Balances;
    type Extra = ();
    type ForceOrigin = EnsureRoot<AccountId>;
    type Freezer = ();
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type RemoveItemsLimit = RemoveItemsLimit;
    type RuntimeEvent = RuntimeEvent;
    type StringLimit = StringLimit;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_assets::WeightInfo<Runtime>;
}

parameter_types! {
    /// Relay Chain `TransactionByteFee` / 10
    pub const TransactionByteFee: Balance = 10 * MICROCENTS;
    pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
    /// There are two possible mechanisms available: slow and fast adjusting.
    /// With slow adjusting fees stay almost constant in short periods of time, changing only in long term.
    /// It may lead to long inclusion times during spikes, therefore tipping is enabled.
    /// With fast adjusting fees change rapidly, but fixed for all users at each block (no tipping)
    type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type RuntimeEvent = RuntimeEvent;
    type WeightToFee = WeightToFee;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_sudo::WeightInfo<Runtime>;
}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
    #[cfg(not(feature = "async-backing"))]
    type CheckAssociatedRelayNumber = RelayNumberStrictlyIncreases;
    #[cfg(feature = "async-backing")]
    type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
    type ConsensusHook = ConsensusHook;
    type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
    type OnSystemEvent = ();
    type OutboundXcmpMessageSource = XcmpQueue;
    type ReservedDmpWeight = ReservedDmpWeight;
    type ReservedXcmpWeight = ReservedXcmpWeight;
    type RuntimeEvent = RuntimeEvent;
    type SelfParaId = parachain_info::Pallet<Runtime>;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::cumulus_pallet_parachain_system::WeightInfo<Runtime>;
    type XcmpMessageHandler = XcmpQueue;
}

impl parachain_info::Config for Runtime {}

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

impl cumulus_pallet_aura_ext::Config for Runtime {}

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

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
}

parameter_types! {
    // pallet_session ends the session after a fixed period of blocks.
    // The first session will have length of Offset,
    // and the following sessions will have length of Period.
    // This may prove nonsensical if Offset >= Period.
    pub const Period: u32 = 6 * HOURS;
    pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
    type Keys = SessionKeys;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type RuntimeEvent = RuntimeEvent;
    // Essentially just Aura, but let's be pedantic.
    type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
    type SessionManager = CollatorSelection;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    // we don't have stash and controller, thus we don't need the convert as well.
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
}

#[cfg(not(feature = "async-backing"))]
parameter_types! {
    pub const AllowMultipleBlocksPerSlot: bool = false;
    pub const MaxAuthorities: u32 = 100_000;
}

#[cfg(feature = "async-backing")]
parameter_types! {
    pub const AllowMultipleBlocksPerSlot: bool = true;
    pub const MaxAuthorities: u32 = 100_000;
}

impl pallet_aura::Config for Runtime {
    type AllowMultipleBlocksPerSlot = AllowMultipleBlocksPerSlot;
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = MaxAuthorities;
    type SlotDuration = ConstU64<SLOT_DURATION>;
}

parameter_types! {
    pub const PotId: PalletId = PalletId(*b"PotStake");
    pub const SessionLength: BlockNumber = 6 * HOURS;
    // StakingAdmin pluralistic body.
    pub const StakingAdminBodyId: BodyId = BodyId::Defense;
    pub const MaxCandidates: u32 = 100;
    pub const MaxInvulnerables: u32 = 20;
    pub const MinEligibleCollators: u32 = 4;
}

impl pallet_collator_selection::Config for Runtime {
    type Currency = Balances;
    // should be a multiple of session or things will get inconsistent
    type KickThreshold = Period;
    type MaxCandidates = MaxCandidates;
    type MaxInvulnerables = MaxInvulnerables;
    type MinEligibleCollators = MinEligibleCollators;
    type PotId = PotId;
    type RuntimeEvent = RuntimeEvent;
    type UpdateOrigin = CollatorSelectionUpdateOrigin;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ValidatorRegistration = Session;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_collator_selection::WeightInfo<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
    pub LocationParents: u8 = 1;
    pub BenchmarkParaId: u8 = 0;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = 2 * GRAND;
    pub const ProposalBondMaximum: Balance = GRAND;
    pub const SpendPeriod: BlockNumber = 6 * DAYS;
    pub const Burn: Permill = Permill::from_perthousand(2);
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;
    // The asset's interior location for the paying account. This is the Treasury
    // pallet instance (which sits at index 13).
    pub TreasuryInteriorLocation: InteriorLocation = PalletInstance(13).into();
    pub const MaxApprovals: u32 = 100;
    pub TreasuryAccount: AccountId = Treasury::account_id();
}

impl pallet_treasury::Config for Runtime {
    type ApproveOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
    type AssetKind = AssetKind;
    type BalanceConverter = frame_support::traits::tokens::UnityAssetBalanceConversion;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = polkadot_runtime_common::impls::benchmarks::TreasuryArguments<
        LocationParents,
        BenchmarkParaId,
    >;
    type Beneficiary = Beneficiary;
    type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
    type Burn = ();
    type BurnDestination = ();
    type Currency = Balances;
    type MaxApprovals = MaxApprovals;
    type OnSlash = Treasury;
    type PalletId = TreasuryPalletId;
    #[cfg(feature = "runtime-benchmarks")]
    type Paymaster = PayWithEnsure<TreasuryPaymaster, OpenHrmpChannel<BenchmarkParaId>>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Paymaster = TreasuryPaymaster;
    type PayoutPeriod = PayoutSpendPeriod;
    type ProposalBond = ProposalBond;
    type ProposalBondMaximum = ProposalBondMaximum;
    type ProposalBondMinimum = ProposalBondMinimum;
    type RejectOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
    type RuntimeEvent = RuntimeEvent;
    type SpendFunds = ();
    type SpendOrigin = TreasurySpender;
    type SpendPeriod = SpendPeriod;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
}
