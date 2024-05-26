use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// High-level currency categories supported on this chain
// TODO: add Erc20 variant like Moonbeam which calls contract_address in EVM via ERC 20 interface
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum CurrencyId {
    // Our native token
    Native,
    // Assets representing other chain's native tokens
    ForeignAsset(AssetId),
}

parameter_types! {
    pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Native;
}

// TODO: keep custom pallet in pallets until successfull upstreams
// pa
// MultiCurrency trait bounds to satisfy with pallet to upstream
// TransferAll<Self::AccountId>
// + MultiCurrencyExtended<Self::AccountId>
// + MultiLockableCurrency<Self::AccountId>
// + MultiReservableCurrency<Self::AccountId>
// + NamedMultiReservableCurrency<Self::AccountId>;

impl orml_currencies::Config for Runtime {
    type GetNativeCurrencyId = GetNativeCurrencyId;
    // TODO: make this work or impl MultiCurrency trait for runtime using Assets + Balances
    // > consider extracting this and other configs out into own defi file
    type MultiCurrency = ();
    // TODO: impl Assets + Balances for this functionality
    type NativeCurrency =
        orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type WeightInfo = (); // TODO: run weights for this runtime and use generated weights
}

// parameter_types! {
//     // TODO: make sure this is Balances pallet AssetId everywhere else
//     pub const NativeAssetId : AssetId = 0;
// }
////UNCOMMENT ONLY AFTER construct_runtime part uncommented
// impl pallet_transaction_multi_payment::Config for Runtime {
//     type AcceptedCurrencyOrigin = EnsureRoot<AccountId>;
//     // TODO: impl orml-currency to combine pallet-asset + pallet-balances
//     // for a sensible MultiCurrency impl
//     // or impl MultiCurrency by runtime using pallet-asset + pallet-balances
//     type Currencies = Currencies;
//     //Currencies
//  // TODO: ensure matches EVM used in `pallet_evm`
//     type EvmAssetId = evm::WethAssetId;
//     type EvmPermit = evm::permit::EvmPermitHandler<Runtime>;
//     // TODO: impl InspectEVMAccounts by Runtime in separate folder
//     // because we do not require pallet-evm-accounts config
//     type InspectEvmAccounts = ();
//     //EVMAccounts
//  //TODO
//     type NativeAssetId = ();
//     //NativeAssetId
//     type OraclePriceProvider = ();
//     //OraclePriceProvider<AssetId, EmaOracle, LRNA>;
//     type RouteProvider = ();
//     //Router;
//     type RuntimeEvent = RuntimeEvent;
//     type TryCallCurrency<'a> = pallet_transaction_multi_payment::TryCallCurrency<Runtime>;
//     type WeightInfo = ();
//     //TODO: run weights in context of this runtime, first add to benchmarking runtime config
//     type WeightToFee = WeightToFee;
// }
