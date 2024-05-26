// TODO: make issue to upstream as follow-up to this PR
// This pallet sensibly routes asset operation execution to pallet-balances for the native asset and to pallet-assets for foreign assets.
//
// The specific motivaton of this pallet was to satisfy the trait bounds of orml_currencues::Config::MultiCurrency such that `orml_currencies` routes asset operation execution to pallet-balances for the native asset and pallet-assets for foreign assets.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
    pallet_prelude::*,
    traits::{
        Currency as PalletCurrency, ExistenceRequirement, Get, Imbalance,
        LockableCurrency as PalletLockableCurrency,
        NamedReservableCurrency as PalletNamedReservableCurrency,
        ReservableCurrency as PalletReservableCurrency, WithdrawReasons,
    },
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use orml_traits::{
    arithmetic::{Signed, SimpleArithmetic},
    currency::TransferAll,
    BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency,
    BasicReservableCurrency, LockIdentifier, MultiCurrency, MultiCurrencyExtended,
    MultiLockableCurrency, MultiReservableCurrency, NamedBasicReservableCurrency,
    NamedMultiReservableCurrency,
};
use orml_utilities::with_transaction_result;
use parity_scale_codec::Codec;
use sp_runtime::{
    traits::{CheckedSub, MaybeSerializeDeserialize, StaticLookup, Zero},
    DispatchError, DispatchResult,
};
use sp_std::{fmt::Debug, marker, result};

// mod mock;
// mod tests;
// mod weights;

// pub use module::*;
// pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_assets::Config + pallet_balances::Config
    {
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
    fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
        with_transaction_result(|| {
            // transfer non-native free to dest
            // TODO: use assets pallet here
            T::MultiCurrency::transfer_all(source, dest)?;

            // transfer all free to dest
            // TODO: use balances pallet here
            T::NativeCurrency::transfer(source, dest, T::NativeCurrency::free_balance(source))
        })
    }
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    // TODO: impl using pallet-assets or pallet-balances
    type Amount = AmountOf<T>;

    fn update_balance(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        by_amount: Self::Amount,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::update_balance(who, by_amount)
        } else {
            T::MultiCurrency::update_balance(currency_id, who, by_amount)
        }
    }
}

impl<T: Config> MultiLockableCurrency<T::AccountId> for Pallet<T> {
    type Moment = BlockNumberFor<T>;

    fn set_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::set_lock(lock_id, who, amount)
        } else {
            T::MultiCurrency::set_lock(lock_id, currency_id, who, amount)
        }
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::extend_lock(lock_id, who, amount)
        } else {
            T::MultiCurrency::extend_lock(lock_id, currency_id, who, amount)
        }
    }

    fn remove_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::remove_lock(lock_id, who)
        } else {
            T::MultiCurrency::remove_lock(lock_id, currency_id, who)
        }
    }
}

impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
    fn can_reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> bool {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::can_reserve(who, value)
        } else {
            T::MultiCurrency::can_reserve(currency_id, who, value)
        }
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::slash_reserved(who, value)
        } else {
            T::MultiCurrency::slash_reserved(currency_id, who, value)
        }
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::reserved_balance(who)
        } else {
            T::MultiCurrency::reserved_balance(currency_id, who)
        }
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::reserve(who, value)
        } else {
            T::MultiCurrency::reserve(currency_id, who, value)
        }
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::unreserve(who, value)
        } else {
            T::MultiCurrency::unreserve(currency_id, who, value)
        }
    }

    fn repatriate_reserved(
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::repatriate_reserved(slashed, beneficiary, value, status)
        } else {
            T::MultiCurrency::repatriate_reserved(currency_id, slashed, beneficiary, value, status)
        }
    }
}

impl<T: Config> NamedMultiReservableCurrency<T::AccountId> for Pallet<T> {
    type ReserveIdentifier = ReserveIdentifierOf<T>;

    fn slash_reserved_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::slash_reserved_named(id, who, value)
        } else {
            T::MultiCurrency::slash_reserved_named(id, currency_id, who, value)
        }
    }

    fn reserved_balance_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::reserved_balance_named(id, who)
        } else {
            T::MultiCurrency::reserved_balance_named(id, currency_id, who)
        }
    }

    fn reserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::reserve_named(id, who, value)
        } else {
            T::MultiCurrency::reserve_named(id, currency_id, who, value)
        }
    }

    fn unreserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::unreserve_named(id, who, value)
        } else {
            T::MultiCurrency::unreserve_named(id, currency_id, who, value)
        }
    }

    fn repatriate_reserved_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        if currency_id == T::GetNativeCurrencyId::get() {
            T::NativeCurrency::repatriate_reserved_named(id, slashed, beneficiary, value, status)
        } else {
            T::MultiCurrency::repatriate_reserved_named(
                id,
                currency_id,
                slashed,
                beneficiary,
                value,
                status,
            )
        }
    }
}
