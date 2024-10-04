pub mod asset_config;
pub use asset_config::AssetType;
pub mod governance;
pub mod xcm_config;

#[cfg(feature = "async-backing")]
use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
#[cfg(not(feature = "async-backing"))]
use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, AssetId, ParaId};
use frame_support::{
    derive_impl,
    dispatch::DispatchClass,
    parameter_types,
    traits::{
        ConstU32, ConstU64, Contains, EitherOfDiverse, FindAuthor, InstanceFilter, TransformOrigin,
    },
    weights::{ConstantMultiplier, Weight},
    PalletId,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot,
};
pub use governance::origins::pallet_custom_origins;
use governance::{origins::Treasurer, TreasurySpender};
use pallet_ethereum::PostLogContent;
use pallet_evm::{EVMCurrencyAdapter, EnsureAccountId20, IdentityAddressMapping};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use polkadot_runtime_wrappers::{impl_openzeppelin_system, SystemConfig};
use scale_info::TypeInfo;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{H160, U256};
use sp_runtime::{
    traits::{AccountIdLookup, BlakeTwo256, IdentityLookup},
    ConsensusEngineId, Perbill, Permill, RuntimeDebug,
};
use sp_std::marker::PhantomData;
use sp_version::RuntimeVersion;
// XCM Imports
use xcm::latest::{prelude::BodyId, InteriorLocation, Junction::PalletInstance};
#[cfg(not(feature = "runtime-benchmarks"))]
use xcm_builder::ProcessXcmMessage;
use xcm_config::{RelayLocation, XcmOriginToTransactDispatchOrigin};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmark::{OpenHrmpChannel, PayWithEnsure};
#[cfg(feature = "async-backing")]
use crate::constants::SLOT_DURATION;
use crate::{
    constants::{
        currency::{deposit, CENTS, EXISTENTIAL_DEPOSIT, GRAND, MICROCENTS},
        AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MAX_BLOCK_LENGTH,
        NORMAL_DISPATCH_RATIO, VERSION, WEIGHT_PER_GAS,
    },
    opaque,
    types::{
        AccountId, AssetKind, Balance, Beneficiary, Block, BlockNumber,
        CollatorSelectionUpdateOrigin, ConsensusHook, Hash, Nonce,
        PriceForSiblingParachainDelivery, TreasuryPaymaster,
    },
    weights::{self, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    Aura, Balances, BaseFee, CollatorSelection, EVMChainId, MessageQueue, OpenZeppelinPrecompiles,
    OriginCaller, PalletInfo, ParachainSystem, Preimage, Runtime, RuntimeCall, RuntimeEvent,
    RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask, Session, SessionKeys,
    System, Timestamp, Treasury, UncheckedExtrinsic, WeightToFee, XcmpQueue,
};

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
    pub const Version: RuntimeVersion = VERSION;
    // generic substrate prefix. For more info, see: [Polkadot Accounts In-Depth](https://wiki.polkadot.network/docs/learn-account-advanced#:~:text=The%20address%20format%20used%20in,belonging%20to%20a%20specific%20network)
    pub const SS58Prefix: u16 = 42;
}
/// OpenZeppelin configuration
pub struct OpenZeppelinSystemConfig;
impl SystemConfig for OpenZeppelinSystemConfig {
    type AccountId = AccountId;
    type ExistentialDeposit = ExistentialDeposit;
    type PreimageOrigin = EnsureRoot<AccountId>;
    type SS58Prefix = SS58Prefix;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type Version = Version;
}
impl_openzeppelin_system!(OpenZeppelinSystemConfig);

impl pallet_authorship::Config for Runtime {
    type EventHandler = (CollatorSelection,);
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
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
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
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
    type MaxActiveOutboundChannels = ConstU32<128>;
    type MaxInboundSuspended = MaxInboundSuspended;
    type MaxPageSize = ConstU32<{ 1 << 16 }>;
    /// Ensure that this value is not set to null (or NoPriceForMessageDelivery) to prevent spamming
    type PriceForSiblingDelivery = PriceForSiblingParachainDelivery;
    type RuntimeEvent = RuntimeEvent;
    type VersionWrapper = ();
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
    // Enqueue XCMP messages from siblings for later processing.
    type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
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
    #[cfg(feature = "async-backing")]
    type SlotDuration = ConstU64<SLOT_DURATION>;
    #[cfg(not(feature = "async-backing"))]
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Self>;
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
    type WeightInfo = weights::pallet_collator_selection::WeightInfo<Runtime>;
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

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
    pub LocationParents: u8 = 1;
    pub BenchmarkParaId: u8 = 0;
}

impl pallet_treasury::Config for Runtime {
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
    type PalletId = TreasuryPalletId;
    #[cfg(feature = "runtime-benchmarks")]
    type Paymaster = PayWithEnsure<TreasuryPaymaster, OpenHrmpChannel<BenchmarkParaId>>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Paymaster = TreasuryPaymaster;
    type PayoutPeriod = PayoutSpendPeriod;
    type RejectOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
    type RuntimeEvent = RuntimeEvent;
    type SpendFunds = ();
    type SpendOrigin = TreasurySpender;
    type SpendPeriod = SpendPeriod;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
}

parameter_types! {
    pub const PostBlockAndTxnHashes: PostLogContent = PostLogContent::BlockAndTxnHashes;
}

impl pallet_ethereum::Config for Runtime {
    type ExtraDataLength = ConstU32<30>;
    type PostLogContent = PostBlockAndTxnHashes;
    type RuntimeEvent = RuntimeEvent;
    type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
}

parameter_types! {
    /// Block gas limit is calculated with target for 75% of block capacity and ratio of maximum block weight and weight per gas
    pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);
    /// To calculate ratio of Gas Limit to PoV size we take the BlockGasLimit we calculated before, and divide it on MAX_POV_SIZE
    pub GasLimitPovSizeRatio: u64 = BlockGasLimit::get().min(u64::MAX.into()).low_u64().saturating_div(cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64);
    pub PrecompilesValue: OpenZeppelinPrecompiles<Runtime> = OpenZeppelinPrecompiles::<_>::new();
    pub WeightPerGas: Weight = Weight::from_parts(WEIGHT_PER_GAS, 0);
    pub SuicideQuickClearLimit: u32 = 0;
}

impl pallet_evm::Config for Runtime {
    type AddressMapping = IdentityAddressMapping;
    type BlockGasLimit = BlockGasLimit;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type ChainId = EVMChainId;
    type Currency = Balances;
    type FeeCalculator = BaseFee;
    type FindAuthor = FindAuthorSession<Aura>;
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type OnChargeTransaction = EVMCurrencyAdapter<Balances, ()>;
    type OnCreate = ();
    type PrecompilesType = OpenZeppelinPrecompiles<Self>;
    type PrecompilesValue = PrecompilesValue;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type RuntimeEvent = RuntimeEvent;
    type SuicideQuickClearLimit = SuicideQuickClearLimit;
    type Timestamp = Timestamp;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_evm::WeightInfo<Self>;
    type WeightPerGas = WeightPerGas;
    type WithdrawOrigin = EnsureAccountId20;
}

impl pallet_evm_chain_id::Config for Runtime {}

parameter_types! {
    /// Starting value for base fee. Set at the same value as in Ethereum.
    pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000);
    /// Default elasticity rate. Set at the same value as in Ethereum.
    pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}

/// The thresholds based on which the base fee will change.
pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
    fn lower() -> Permill {
        Permill::zero()
    }

    fn ideal() -> Permill {
        Permill::from_parts(500_000)
    }

    fn upper() -> Permill {
        Permill::from_parts(1_000_000)
    }
}
impl pallet_base_fee::Config for Runtime {
    type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
    type DefaultElasticity = DefaultElasticity;
    type RuntimeEvent = RuntimeEvent;
    type Threshold = BaseFeeThreshold;
}

pub struct FindAuthorSession<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorSession<F> {
    fn find_author<'a, I>(digests: I) -> Option<H160>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        if let Some(author_index) = F::find_author(digests) {
            let account_id: AccountId = Session::validators()[author_index as usize];
            return Some(H160::from(account_id));
        }
        None
    }
}

#[derive(Clone)]
pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        )
    }
}

impl fp_rpc::ConvertTransaction<opaque::UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(
        &self,
        transaction: pallet_ethereum::Transaction,
    ) -> opaque::UncheckedExtrinsic {
        let extrinsic = UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        );
        let encoded = extrinsic.encode();
        opaque::UncheckedExtrinsic::decode(&mut &encoded[..])
            .expect("Encoded extrinsic is always valid")
    }
}
