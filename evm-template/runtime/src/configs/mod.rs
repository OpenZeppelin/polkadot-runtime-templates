pub mod asset_config;
pub mod governance;
pub mod xcm_config;

#[cfg(feature = "runtime-benchmarks")]
use asset_config::BenchmarkHelper;
use asset_config::{ApprovalDeposit, AssetAccountDeposit, AssetDeposit};
pub use asset_config::{AssetType, TransactionByteFee};
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
        AsEnsureOriginWithArg, ConstU32, ConstU64, Contains, EitherOf, EitherOfDiverse, Everything,
        FindAuthor, InstanceFilter, Nothing, TransformOrigin,
    },
    weights::{ConstantMultiplier, Weight},
    PalletId,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
pub use governance::origins::pallet_custom_origins;
use governance::{origins::Treasurer, tracks, Spender, WhitelistedCaller};
use pallet_ethereum::PostLogContent;
use pallet_evm::{EVMCurrencyAdapter, EnsureAccountId20, IdentityAddressMapping};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use polkadot_runtime_wrappers::{
    impl_openzeppelin_consensus, impl_openzeppelin_evm, impl_openzeppelin_governance,
    impl_openzeppelin_system, impl_openzeppelin_xcm, ConsensusConfig, EvmConfig, GovernanceConfig,
    SystemConfig, XcmConfig,
};
use scale_info::TypeInfo;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{H160, U256};
use sp_runtime::{
    traits::{AccountIdLookup, BlakeTwo256, IdentityLookup},
    ConsensusEngineId, Perbill, Permill, RuntimeDebug,
};
use sp_std::marker::PhantomData;
use sp_version::RuntimeVersion;
use xcm::latest::{prelude::*, InteriorLocation, Junction::PalletInstance};
#[cfg(not(feature = "runtime-benchmarks"))]
use xcm_builder::ProcessXcmMessage;
use xcm_builder::{
    AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom,
    ConvertedConcreteId, DenyReserveTransferToRelayChain, DenyThenTry, EnsureXcmOrigin,
    FixedWeightBounds, FrameTransactionalProcessor, FungibleAdapter, FungiblesAdapter, HandleFee,
    IsChildSystemParachain, IsConcrete, NativeAsset, NoChecking, ParentIsPreset,
    RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
    SignedAccountKey20AsNative, SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId,
    UsingComponents, WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
// XCM imports
use xcm_config::{
    AssetTransactors, BalancesPalletLocation, FeeManager, LocalOriginToLocation,
    LocationToAccountId, XcmOriginToTransactDispatchOrigin,
};
use xcm_executor::{
    traits::{FeeReason, JustTry, TransactAsset},
    XcmExecutor,
};
use xcm_primitives::AsAssetType;

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
        PriceForSiblingParachainDelivery, TreasuryPaymaster, XcmFeesToAccount,
    },
    weights::{self, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    AllPalletsWithSystem, AssetManager, Aura, Balances, BaseFee, CollatorSelection, EVMChainId,
    MessageQueue, OpenZeppelinPrecompiles, OriginCaller, PalletInfo, ParachainInfo,
    ParachainSystem, PolkadotXcm, Preimage, Referenda, Runtime, RuntimeCall, RuntimeEvent,
    RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask, Scheduler, Session,
    SessionKeys, System, Timestamp, Treasury, UncheckedExtrinsic, WeightToFee, XcmpQueue,
};

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
    pub const Version: RuntimeVersion = VERSION;
    // generic substrate prefix. For more info, see: [Polkadot Accounts In-Depth](https://wiki.polkadot.network/docs/learn-account-advanced#:~:text=The%20address%20format%20used%20in,belonging%20to%20a%20specific%20network)
    pub const SS58Prefix: u16 = 42;
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
    type Trader = (
        UsingComponents<WeightToFee, BalancesPalletLocation, AccountId, Balances, ()>,
        xcm_primitives::FirstAssetTrader<AssetType, AssetManager, XcmFeesToAccount>,
    );
    type XcmAdminOrigin = EnsureRoot<AccountId>;
    type XcmOriginToTransactDispatchOrigin = XcmOriginToTransactDispatchOrigin;
    type XcmpQueueControllerOrigin = EnsureRoot<AccountId>;
    type XcmpQueueMaxInboundSuspended = MaxInboundSuspended;
}
parameter_types! {
    pub PrecompilesValue: OpenZeppelinPrecompiles<Runtime> = OpenZeppelinPrecompiles::<_>::new();
}
impl EvmConfig for OpenZeppelinConfig {
    type AddressMapping = IdentityAddressMapping;
    type CallOrigin = EnsureAccountId20;
    type FindAuthor = FindAuthorSession<Aura>;
    type PrecompilesType = OpenZeppelinPrecompiles<Runtime>;
    type PrecompilesValue = PrecompilesValue;
    type WithdrawOrigin = EnsureAccountId20;
}
impl_openzeppelin_system!(OpenZeppelinConfig);
impl_openzeppelin_consensus!(OpenZeppelinConfig);
impl_openzeppelin_governance!(OpenZeppelinConfig);
impl_openzeppelin_xcm!(OpenZeppelinConfig);
impl_openzeppelin_evm!(OpenZeppelinConfig);

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
