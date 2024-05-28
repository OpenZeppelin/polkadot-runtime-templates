//! This pallet implements MultiCurrency trait bounds for pallet-assets
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
    fungibles::{Balanced, Inspect, InspectEnumerable},
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

// Errors:
// - the trait `std::marker::Copy` is not implemented for `<T as pallet_assets::Config>::AssetId` --> patch: convert using associated type)
// -  the trait `orml_traits::arithmetic::Signed` is not implemented for `<T as pallet_assets::Config>::Balance` --> look ar Signed definition to decide what to do
//  - pallet_assets has transfer, not transfer_all so it must be implemented here or upstream --> iterate through all asset_ids held by the owner?

#[frame_support::pallet]
pub mod module {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_assets::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
    fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
        with_transaction_result(|| {
            for asset_id in pallet_assets::Pallet::<T>::asset_ids() {
                if let Some(balance) = pallet_assets::Pallet::<T>::maybe_balance(asset_id, source) {
                    pallet_assets::Pallet::<T>::transfer(asset_id, source, dest, balance)?;
                }
            }
        })
    }
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
    type Balance = <T as pallet_assets::Config>::Balance;
    type CurrencyId = <T as pallet_assets::Config>::AssetId;

    fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
        pallet_assets::Pallet::<T>::minimum_balance(currency_id)
    }

    fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
        pallet_assets::Pallet::<T>::total_issuance(currency_id)
    }

    fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        pallet_assets::Pallet::<T>::total_balance(currency_id, who)
    }

    fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        pallet_assets::Pallet::<T>::free_balance(currency_id, who)
    }

    fn ensure_can_withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::ensure_can_withdraw(currency_id, who, amount)
    }

    fn transfer(
        currency_id: Self::CurrencyId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::transfer(currency_id, from, to, amount)
    }

    fn deposit(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::deposit(currency_id, who, amount)
    }

    fn withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::withdraw(currency_id, who, amount)
    }

    fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
        pallet_assets::Pallet::<T>::can_slash(currency_id, who, amount)
    }

    fn slash(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::slash(currency_id, who, amount)
    }
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    type Amount = <T as pallet_assets::Config>::Balance;

    fn update_balance(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        by_amount: Self::Amount,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::update_balance(currency_id, who, by_amount)
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
        pallet_assets::Pallet::<T>::set_lock(lock_id, currency_id, who, amount)
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::extend_lock(lock_id, currency_id, who, amount)
    }

    fn remove_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::remove_lock(lock_id, currency_id, who)
    }
}

// TODO: use freeze, thaw, burn, and mint
// to fulfill the required functionality
impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
    fn can_reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> bool {
        pallet_assets::Pallet::<T>::can_reserve(currency_id, who, value)
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::slash_reserved(currency_id, who, value)
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        pallet_assets::Pallet::<T>::reserved_balance(currency_id, who)
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::reserve(currency_id, who, value)
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::unreserve(currency_id, who, value)
    }

    fn repatriate_reserved(
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        pallet_assets::Pallet::<T>::repatriate_reserved(
            currency_id,
            slashed,
            beneficiary,
            value,
            status,
        )
    }
}

impl<T: Config> NamedMultiReservableCurrency<T::AccountId> for Pallet<T> {
    type ReserveIdentifier = ();

    fn slash_reserved_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::slash_reserved_named(id, currency_id, who, value)
    }

    fn reserved_balance_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::reserved_balance_named(id, currency_id, who)
    }

    fn reserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::reserve_named(id, currency_id, who, value)
    }

    fn unreserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::unreserve_named(id, currency_id, who, value)
    }

    fn repatriate_reserved_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
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
