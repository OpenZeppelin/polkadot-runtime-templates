//! Operators pallet
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use confidential_assets_primitives::OperatorRegistry;
use frame_support::{pallet_prelude::*, traits::Get, Blake2_128Concat};
use frame_system::pallet_prelude::*;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier used by the consuming pallets.
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen;

        /// Max operators per (holder, asset) you’re willing to store, if you add limits later.
        /// Not used in logic now; included to make future migrations non-breaking.
        type MaxOperatorsPerAsset: Get<u32>;

        /// Weight info (placeholder).
        type WeightInfo: WeightData;
    }

    pub trait WeightData {
        fn set_operator() -> Weight;
        fn revoke_operator() -> Weight;
        fn clear_operators() -> Weight;
    }
    impl WeightData for () {
        fn set_operator() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn revoke_operator() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn clear_operators() -> Weight {
            Weight::from_parts(10_000, 0)
        }
    }

    /// (holder, asset, operator) -> until_block
    #[pallet::storage]
    pub type Operators<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, T::AssetId>,
            NMapKey<Blake2_128Concat, T::AccountId>,
        ),
        BlockNumberFor<T>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        OperatorSet {
            asset: T::AssetId,
            holder: T::AccountId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        },
        OperatorRevoked {
            asset: T::AssetId,
            holder: T::AccountId,
            operator: T::AccountId,
        },
        OperatorsCleared {
            holder: T::AccountId,
            asset: T::AssetId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        NotHolder,
        NoSuchOperator,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    impl<T: Config> Pallet<T> {
        /// Public helper for other pallets.
        pub fn is_operator(
            holder: &T::AccountId,
            asset: &T::AssetId,
            operator: &T::AccountId,
            now: BlockNumberFor<T>,
        ) -> bool {
            match Operators::<T>::get((holder, asset, operator)) {
                Some(until) => now <= until,
                None => false,
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Holder grants or extends an operator for a specific asset until `until`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_operator())]
        pub fn set_operator(
            origin: OriginFor<T>,
            asset: T::AssetId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            Operators::<T>::insert((holder.clone(), asset, operator.clone()), until);
            Self::deposit_event(Event::OperatorSet {
                asset,
                holder,
                operator,
                until,
            });
            Ok(())
        }

        /// Holder revokes a specific operator for an asset.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revoke_operator())]
        pub fn revoke_operator(
            origin: OriginFor<T>,
            asset: T::AssetId,
            operator: T::AccountId,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            let key = (holder.clone(), asset, operator.clone());
            ensure!(
                Operators::<T>::contains_key(&key),
                Error::<T>::NoSuchOperator
            );
            Operators::<T>::remove(key);
            Self::deposit_event(Event::OperatorRevoked {
                asset,
                holder,
                operator,
            });
            Ok(())
        }

        /// Root can clear *all* operators for a holder/asset (operational escape hatch).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::clear_operators())]
        pub fn clear_operators_root(
            origin: OriginFor<T>,
            holder: T::AccountId,
            asset: T::AssetId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // Remove every entry with the (holder, asset, *) prefix.
            let _ = Operators::<T>::clear_prefix((holder.clone(), asset), u32::MAX, None);
            Self::deposit_event(Event::OperatorsCleared { holder, asset });
            Ok(())
        }
    }
}

/// Implement the trait so other pallets can depend only on `OperatorRegistry`.
impl<T: pallet::Config> OperatorRegistry<T::AccountId, T::AssetId, BlockNumberFor<T>>
    for pallet::Pallet<T>
{
    fn is_operator(
        holder: &T::AccountId,
        asset: &T::AssetId,
        operator: &T::AccountId,
        now: BlockNumberFor<T>,
    ) -> bool {
        <pallet::Pallet<T>>::is_operator(holder, asset, operator, now)
    }
}
