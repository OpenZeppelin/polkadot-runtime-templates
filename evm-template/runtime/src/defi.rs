//! DeFi runtime configurations
use frame_support::parameter_types;
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;

use super::{
    AccountId, Balance, Balances, BlockNumber, Currencies, Runtime, RuntimeEvent, Tokens,
    WeightToFee,
};

pub type Amount = i128;
pub type AssetId = u32;

parameter_types! {
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
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
    // TODO: enforce everywhere, including XCM execution
    pub const NativeAssetId: AssetId = 0u32;
    // NOTE: the underlying impl of `reset_payment_currency` extrinsic assumes that this is not NativeAssetId
    // even if we want to use pallet-balances for managing the EVM Asset
    // TODO: set `EvmAssetId = NativeAssetId` without violating the assumptions of `reset_payment_currency` extrinsic
    // ISSUE #5575
    pub const EvmAssetId: AssetId = 1u32;
}

impl orml_currencies::Config for Runtime {
    type GetNativeCurrencyId = NativeAssetId;
    type MultiCurrency = Tokens;
    type NativeCurrency =
        orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type WeightInfo = (); // TODO: generate weights
}

// TODO: upstream for all EVM chains that use native 20 byte accounts
pub struct AllEvmAccounts;
impl<T> InspectEvmAccounts<AccountId, AccountId> for AllEvmAccounts {
    /// Returns `True` if the account is EVM truncated account.
    fn is_evm_account(account_id: AccountId) -> bool {
        true
    }

    /// get the EVM address from the substrate address.
    fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress {
        // unused in pallet-tx-multi-payment
        [0u8; 20]
    }

    /// Get the truncated address from the EVM address.
    fn truncated_account_id(evm_address: AccountId) -> AccountId {
        evm_address
    }

    /// Return the Substrate address bound to the EVM account. If not bound, returns `None`.
    fn bound_account_id(evm_address: AccountId) -> Option<AccountId> {
        Some(evm_address)
    }

    /// Get the Substrate address from the EVM address.
    /// Returns the truncated version of the address if the address wasn't bind.
    fn account_id(evm_address: AccountId) -> AccountId {
        evm_address
    }

    /// Returns `True` if the address is allowed to deploy smart contracts.
    fn can_deploy_contracts(evm_address: AccountId) -> bool {
        true
    }
}

impl pallet_transaction_multi_payment::Config for Runtime {
    type AcceptedCurrencyOrigin = EnsureRoot<Self::AccountId>;
    type Currencies = Currencies;
    // ISSUE #5575
    type EvmAssetId = NativeAssetId;
    // NOTE: impl EvmPermit for () returns false positive results
    // TODO1: replace this impl with dummy patch that returns true negative results
    // TODO2: impl EvmPermit and its precompile in a follow up because the precompile written by HydraDX-node assumes we use their 32 byte keys (which we do not)
    type EvmPermit = ();
    type InspectEvmAccounts = AllEvmAccounts;
    type NativeAssetId = NativeAssetId;
    type OraclePriceProvider = ();
    type RouteProvider = ();
    type RuntimeEvent = RuntimeEvent;
    type TryCallCurrency<'a> = pallet_transaction_multi_payment::TryCallCurrency<Runtime>;
    // TODO: update weights
    type WeightInfo = ();
    type WeightToFee = WeightToFee;
}
