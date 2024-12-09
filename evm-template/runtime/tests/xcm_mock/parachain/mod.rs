mod xcm_config;
use core::marker::PhantomData;

use evm_runtime_template::configs::xcm_config::SignedToAccountId20;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ContainsPair, Everything, Nothing},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use frame_system::EnsureRoot;
use openzeppelin_polkadot_wrappers::{impl_openzeppelin_xcm, XcmConfig};
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

pub struct OpenZeppelinRuntime;
impl XcmConfig for OpenZeppelinRuntime {
    type AccountIdToLocation = AccountIdToLocation<AccountId>;
    type AddSupportedAssetOrigin = EnsureRoot<AccountId>;
    type AssetFeesFilter = AssetFeesFilter;
    type AssetTransactors = AssetTransactors;
    type BaseXcmWeight = BaseXcmWeight;
    type CurrencyId = CurrencyId;
    type CurrencyIdToLocation = CurrencyIdToLocation<AsAssetType<AssetId, AssetType, AssetManager>>;
    type DerivativeAddressRegistrationOrigin = EnsureRoot<AccountId>;
    type EditSupportedAssetOrigin = EnsureRoot<AccountId>;
    type FeeManager = FeeManager;
    type HrmpManipulatorOrigin = EnsureRoot<AccountId>;
    type HrmpOpenOrigin = EnsureRoot<AccountId>;
    type LocalOriginToLocation = LocalOriginToLocation;
    type LocationToAccountId = LocationToAccountId;
    type MaxAssetsForTransfer = MaxAssetsForTransfer;
    type MaxHrmpRelayFee = MaxHrmpRelayFee;
    type MessageQueueHeapSize = ConstU32<{ 64 * 1024 }>;
    type MessageQueueMaxStale = ConstU32<8>;
    type MessageQueueServiceWeight = MessageQueueServiceWeight;
    type ParachainMinFee = ParachainMinFee;
    type PauseSupportedAssetOrigin = EnsureRoot<AccountId>;
    type RelayLocation = RelayLocation;
    type RemoveSupportedAssetOrigin = EnsureRoot<AccountId>;
    type Reserves = Reserves;
    type ResumeSupportedAssetOrigin = EnsureRoot<AccountId>;
    type SelfLocation = SelfLocation;
    type SelfReserve = SelfReserve;
    type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId>;
    type Trader = pallet_xcm_weight_trader::Trader<Runtime>;
    type TransactorReserveProvider = AbsoluteAndRelativeReserve<SelfLocationAbsolute>;
    type Transactors = Transactors;
    type UniversalLocation = UniversalLocation;
    type WeightToFee = WeightToFee;
    type XcmAdminOrigin = EnsureRoot<AccountId>;
    type XcmFeesAccount = TreasuryAccount;
    type XcmOriginToTransactDispatchOrigin = XcmOriginToTransactDispatchOrigin;
    type XcmSender = XcmRouter;
    type XcmWeigher = XcmWeigher;
    type XcmpQueueControllerOrigin = EnsureRoot<AccountId>;
    type XcmpQueueMaxInboundSuspended = ConstU32<1000>;
    type XtokensReserveProviders = ReserveProviders;
}
impl_openzeppelin_xcm!(OpenZeppelinRuntime);

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Balances: pallet_balances,
        MessageQueue: pallet_message_queue,
        XcmpQueue: cumulus_pallet_xcmp_queue,
        CumulusXcm: cumulus_pallet_xcm,
        XcmWeightTrader: pallet_xcm_weight_trader,
        XTokens: orml_xtokens,
        XcmTransactor: pallet_xcm_transactor,
        PolkadotXcm: pallet_xcm,
    }
);
