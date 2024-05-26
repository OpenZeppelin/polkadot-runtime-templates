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
pub use module::*;
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

#[frame_support::pallet]
pub mod module {
    use super::*;

    pub(crate) type BalanceOf<T> = <pallet_balances::Pallet<T> as PalletCurrency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;
    // pub(crate) type CurrencyIdOf<T> =
    // 	<<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
    // pub(crate) type AmountOf<T> =
    // 	<<T as Config>::MultiCurrency as MultiCurrencyExtended<<T as frame_system::Config>::AccountId>>::Amount;
    // pub(crate) type ReserveIdentifierOf<T> = <<T as Config>::MultiCurrency as NamedMultiReservableCurrency<
    // 	<T as frame_system::Config>::AccountId,
    // >>::ReserveIdentifier;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_assets::Config + pallet_balances::Config
    {
        // TODO: CurrencyId should be defined in runtime and only used here in a more generic way
        type NativeCurrencyId: Get<CurrencyId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
    fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
        with_transaction_result(|| {
            // transfer non-native free to dest
            pallet_assets::Pallet::<T>::transfer_all(source, dest)?;

            // transfer all free to dest
            pallet_balances::Pallet::<T>::transfer(
                source,
                dest,
                pallet_balances::Pallet::<T>::free_balance(source),
                ExistenceRequirement::KeepAlive,
            )
        })
    }
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
    type Balance = BalanceOf<T>;
    type CurrencyId = CurrencyIdOf<T>;

    fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::minimum_balance()
        } else {
            pallet_assets::Pallet::<T>::minimum_balance(currency_id)
        }
    }

    fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::total_issuance()
        } else {
            pallet_assets::Pallet::<T>::total_issuance(currency_id)
        }
    }

    fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::total_balance(who)
        } else {
            pallet_assets::Pallet::<T>::total_balance(currency_id, who)
        }
    }

    fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::free_balance(who)
        } else {
            pallet_assets::Pallet::<T>::free_balance(currency_id, who)
        }
    }

    fn ensure_can_withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::ensure_can_withdraw(who, amount)
        } else {
            pallet_assets::Pallet::<T>::ensure_can_withdraw(currency_id, who, amount)
        }
    }

    fn transfer(
        currency_id: Self::CurrencyId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if amount.is_zero() || from == to {
            return Ok(());
        }
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::transfer(from, to, amount)
        } else {
            pallet_assets::Pallet::<T>::transfer(currency_id, from, to, amount)
        }
    }

    fn deposit(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::deposit(who, amount)
        } else {
            pallet_assets::Pallet::<T>::deposit(currency_id, who, amount)
        }
    }

    fn withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::withdraw(who, amount)
        } else {
            pallet_assets::Pallet::<T>::withdraw(currency_id, who, amount)
        }
    }

    fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::can_slash(who, amount)
        } else {
            pallet_assets::Pallet::<T>::can_slash(currency_id, who, amount)
        }
    }

    fn slash(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::slash(who, amount)
        } else {
            pallet_assets::Pallet::<T>::slash(currency_id, who, amount)
        }
    }
}
// TODO: implement MultiCurrency as well
impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    type Amount = AmountOf<T>;

    fn update_balance(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        by_amount: Self::Amount,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::update_balance(who, by_amount)
        } else {
            pallet_assets::Pallet::<T>::update_balance(currency_id, who, by_amount)
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
            pallet_balances::Pallet::<T>::set_lock(lock_id, who, amount)
        } else {
            pallet_assets::Pallet::<T>::set_lock(lock_id, currency_id, who, amount)
        }
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::extend_lock(lock_id, who, amount)
        } else {
            pallet_assets::Pallet::<T>::extend_lock(lock_id, currency_id, who, amount)
        }
    }

    fn remove_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::remove_lock(lock_id, who)
        } else {
            pallet_assets::Pallet::<T>::remove_lock(lock_id, currency_id, who)
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
            pallet_balances::Pallet::<T>::can_reserve(who, value)
        } else {
            pallet_assets::Pallet::<T>::can_reserve(currency_id, who, value)
        }
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::slash_reserved(who, value)
        } else {
            pallet_assets::Pallet::<T>::slash_reserved(currency_id, who, value)
        }
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::reserved_balance(who)
        } else {
            pallet_assets::Pallet::<T>::reserved_balance(currency_id, who)
        }
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::reserve(who, value)
        } else {
            pallet_assets::Pallet::<T>::reserve(currency_id, who, value)
        }
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::unreserve(who, value)
        } else {
            pallet_assets::Pallet::<T>::unreserve(currency_id, who, value)
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
            pallet_balances::Pallet::<T>::repatriate_reserved(slashed, beneficiary, value, status)
        } else {
            pallet_assets::Pallet::<T>::repatriate_reserved(
                currency_id,
                slashed,
                beneficiary,
                value,
                status,
            )
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
            pallet_balances::Pallet::<T>::slash_reserved_named(id, who, value)
        } else {
            pallet_assets::Pallet::<T>::slash_reserved_named(id, currency_id, who, value)
        }
    }

    fn reserved_balance_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::reserved_balance_named(id, who)
        } else {
            pallet_assets::Pallet::<T>::reserved_balance_named(id, currency_id, who)
        }
    }

    fn reserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::reserve_named(id, who, value)
        } else {
            pallet_assets::Pallet::<T>::reserve_named(id, currency_id, who, value)
        }
    }

    fn unreserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        if currency_id == T::GetNativeCurrencyId::get() {
            pallet_balances::Pallet::<T>::unreserve_named(id, who, value)
        } else {
            pallet_assets::Pallet::<T>::unreserve_named(id, currency_id, who, value)
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
            pallet_balances::Pallet::<T>::repatriate_reserved_named(
                id,
                slashed,
                beneficiary,
                value,
                status,
            )
        } else {
            pallet_assets::Pallet::<T>::repatriate_reserved_named(
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
