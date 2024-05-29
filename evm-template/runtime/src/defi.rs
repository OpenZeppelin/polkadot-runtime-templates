//! DeFi runtime configurations
use frame_support::parameter_types;
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;

use super::{
    Balance, Balances, BlockNumber, Currencies, Runtime, RuntimeEvent, Tokens, WeightToFee,
};

pub type Amount = i128;
pub type AssetId = u32;

parameter_types! {
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
        Balance::default()
    };
}

impl orml_tokens::Config for Runtime {
    type Amount = Amount;
    type Balance = Balance;
    type CurrencyHooks = ();
    type CurrencyId = AssetId;
    type DustRemovalWhitelist = ();
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetNativeCurrencyId: AssetId = 0u32;
}

impl orml_currencies::Config for Runtime {
    type GetNativeCurrencyId = GetNativeCurrencyId;
    type MultiCurrency = Tokens;
    type NativeCurrency =
        orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type WeightInfo = (); // TODO: generate weights
}

impl pallet_transaction_multi_payment::Config for Runtime {
    type AcceptedCurrencyOrigin = EnsureRoot<AccountId>;
    type Currencies = Currencies;
    // TODO: ensure matches EVM used in `pallet_evm`
    type EvmAssetId = evm::WethAssetId;
    type EvmPermit = evm::permit::EvmPermitHandler<Runtime>;
    // TODO: impl InspectEVMAccounts by Runtime in separate folder
    // because we do not require pallet-evm-accounts config
    type InspectEvmAccounts = ();
    //EVMAccounts
    type NativeAssetId = GetNativeCurrencyId;
    type OraclePriceProvider = ();
    //OraclePriceProvider<AssetId, EmaOracle, LRNA>;
    type RouteProvider = ();
    //Router;
    type RuntimeEvent = RuntimeEvent;
    type TryCallCurrency<'a> = pallet_transaction_multi_payment::TryCallCurrency<Runtime>;
    type WeightInfo = ();
    //TODO: run weights in context of this runtime, first add to benchmarking runtime config
    type WeightToFee = WeightToFee;
}
