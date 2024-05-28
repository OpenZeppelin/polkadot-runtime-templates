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
        pallet_assets::Pallet::<T>::can_slash(currency_id.into(), who, amount)
    }

    fn slash(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        pallet_assets::Pallet::<T>::slash(currency_id.into(), who, amount.into())
    }
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    type Amount = T::OrmlBalance;

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
        // can_freeze
        pallet_assets::Pallet::<T>::can_reserve(currency_id, who, value)
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        // unfreeze + burn
        pallet_assets::Pallet::<T>::slash_reserved(currency_id, who, value)
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        // frozen_balance
        pallet_assets::Pallet::<T>::reserved_balance(currency_id, who)
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        // freeze
        pallet_assets::Pallet::<T>::reserve(currency_id, who, value)
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        // thaw
        pallet_assets::Pallet::<T>::unreserve(currency_id, who, value)
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
        pallet_assets::Pallet::<T>::repatriate_reserved(
            currency_id,
            slashed,
            beneficiary,
            value,
            status,
        )
    }
}

// check what ReserveIdentifier is for orml_tokens::Config to determine
// what needs to be implemented here
// we may need to store named reserves in a storage map on top of pallet-assets
// impl<T: Config> NamedMultiReservableCurrency<T::AccountId> for Pallet<T> {
//     type ReserveIdentifier = ();

//     fn slash_reserved_named(
//         _id: &Self::ReserveIdentifier,
//         currency_id: Self::CurrencyId,
//         who: &T::AccountId,
//         value: Self::Balance,
//     ) -> Self::Balance {
//         Self::slash_reserved(currency_id, who, value)
//     }

//     fn reserved_balance_named(
//         id: &Self::ReserveIdentifier,
//         currency_id: Self::CurrencyId,
//         who: &T::AccountId,
//     ) -> Self::Balance {
//         Self::reserved_balance(currency_id, who)
//     }

//     fn reserve_named(
//         id: &Self::ReserveIdentifier,
//         currency_id: Self::CurrencyId,
//         who: &T::AccountId,
//         value: Self::Balance,
//     ) -> DispatchResult {
//         Self::reserve(currency_id, who, value)
//     }

//     fn unreserve_named(
//         id: &Self::ReserveIdentifier,
//         currency_id: Self::CurrencyId,
//         who: &T::AccountId,
//         value: Self::Balance,
//     ) -> Self::Balance {
//         Self::unreserve(id, currency_id, who, value)
//     }

//     fn repatriate_reserved_named(
//         id: &Self::ReserveIdentifier,
//         currency_id: Self::CurrencyId,
//         slashed: &T::AccountId,
//         beneficiary: &T::AccountId,
//         value: Self::Balance,
//         status: BalanceStatus,
//     ) -> result::Result<Self::Balance, DispatchError> {
//         Self::repatriate_reserved(
//             currency_id,
//             slashed,
//             beneficiary,
//             value,
//             status,
//         )
//     }
// }
