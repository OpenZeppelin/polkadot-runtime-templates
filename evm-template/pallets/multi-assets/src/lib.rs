//! This pallet implements MultiCurrency trait bounds for pallet-assets thereby enable assignment in an orml_currencies Config alongside pallet-balances.
//! The resulting config is different from the happy path config for orml_currencies::Config::MultiCurrency which uses orml_tokens instead of pallet-assets for orml_currencies::Config::MultiCurrency.
//! Associated types are used to satisfy the orml_currencies strict(er) trait bounds with pallet_asset::Config (more) general associated types.
//! This pallet's associated types may be removed once upstream compatibility between orml_currencies and pallet_assets is improved.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
    pallet_prelude::*,
    traits::{
        fungibles::{Balanced, Inspect, InspectEnumerable, Mutate},
        tokens::{Fortitude, Precision, Preservation},
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
use parity_scale_codec::FullCodec;
use sp_runtime::{
    traits::{CheckedSub, Convert, MaybeSerializeDeserialize, StaticLookup, Zero},
    DispatchError, DispatchResult,
};
use sp_std::{fmt::Debug, marker, result};

#[frame_support::pallet]
pub mod module {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_assets::Config {
        // LocalAssetId
        // TODO: remove once Copy trait bound is removed from orml_currencies::CurrencyId or Clone trait bound is removed from pallet_assets::AssetId
        // - also remove `.into()`s throughout
        type Id: FullCodec
            + Eq
            + PartialEq
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + scale_info::TypeInfo
            + MaxEncodedLen
            + Into<Self::AssetId>
            + From<Self::AssetId>;
        // LocalBalance
        // TODO: remove once `orml_traits::arithmetic::Signed` is implemented and/or enforced as a trait bound for `<T as pallet_assets::Config>::Balance`
        // - also remove `.into()`s throughout
        type OrmlBalance: FullCodec
            + Eq
            + PartialEq
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + scale_info::TypeInfo
            + MaxEncodedLen
            + orml_traits::arithmetic::Signed
            + frame_support::traits::tokens::Balance
            + Into<Self::Balance>
            + From<Self::Balance>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        Unimplemented,
    }
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
    fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
        with_transaction_result(|| {
            for id in pallet_assets::Pallet::<T>::asset_ids() {
                if let Some(balance) = pallet_assets::Pallet::<T>::maybe_balance(id.clone(), source)
                {
                    <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::transfer(
                        id,
                        source,
                        dest,
                        balance,
                        Preservation::Preserve,
                    )
                    .map(|_| ())?;
                }
            }
            Ok(())
        })
    }
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
    type Balance = T::OrmlBalance;
    type CurrencyId = T::Id;

    fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
        pallet_assets::Pallet::<T>::minimum_balance(currency_id.into()).into()
    }

    fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
        pallet_assets::Pallet::<T>::total_issuance(currency_id.into()).into()
    }

    fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        pallet_assets::Pallet::<T>::total_balance(currency_id.into(), who).into()
    }

    fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        pallet_assets::Pallet::<T>::balance(currency_id.into(), who).into()
    }

    fn ensure_can_withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::can_withdraw(currency_id.into(), who, amount.into())
            .into_result(true)
            .map(|_| ())
    }

    fn transfer(
        currency_id: Self::CurrencyId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::transfer(
            currency_id.into(),
            from,
            to,
            amount.into(),
            Preservation::Preserve,
        )
        .map(|_| ())
    }

    fn deposit(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::mint_into(currency_id.into(), who, amount.into()).map(|_| ())
    }

    fn withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        pallet_assets::Pallet::<T>::burn_from(
            currency_id.into(),
            who,
            amount.into(),
            Precision::BestEffort,
            Fortitude::Force,
        )
        .map(|_| ())
    }

    fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
        Self::ensure_can_withdraw(currency_id, who, amount).is_ok()
    }

    fn slash(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::burn_from(
            currency_id.into(),
            who,
            amount.into(),
            Precision::BestEffort,
            Fortitude::Force,
        )
        .unwrap_or(Default::default())
        .into()
    }
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    type Amount = T::OrmlBalance;

    // ~= T::Amount in orml_tokens

    fn update_balance(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        by_amount: Self::Amount,
    ) -> DispatchResult {
        if by_amount.is_positive() {
            Self::deposit(currency_id, who, by_amount)
        } else {
            Self::withdraw(currency_id, who, by_amount).map(|_| ())
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
        Err(Error::<T>::Unimplemented.into())
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        Err(Error::<T>::Unimplemented.into())
    }

    fn remove_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> DispatchResult {
        Err(Error::<T>::Unimplemented.into())
    }
}

impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
    fn can_reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> bool {
        // can_freeze
        false
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        // unfreeze + burn
        Default::default()
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        // frozen_balance
        Default::default()
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        // freeze
        Err(Error::<T>::Unimplemented.into())
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        // thaw
        Default::default()
    }

    fn repatriate_reserved(
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        // unfreeze from slashed
        // transfer to beneficiary
        Err(Error::<T>::Unimplemented.into())
    }
}

impl<T: Config> NamedMultiReservableCurrency<T::AccountId> for Pallet<T> {
    type ReserveIdentifier = ();

    fn slash_reserved_named(
        _id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        Default::default()
    }

    fn reserved_balance_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> Self::Balance {
        Default::default()
    }

    fn reserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        Err(Error::<T>::Unimplemented.into())
    }

    fn unreserve_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        Default::default()
    }

    fn repatriate_reserved_named(
        id: &Self::ReserveIdentifier,
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        Err(Error::<T>::Unimplemented.into())
    }
}
