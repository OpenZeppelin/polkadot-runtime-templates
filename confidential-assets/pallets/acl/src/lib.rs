//! ACL Pallet
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use confidential_assets_primitives::{AclCtx, AclProvider, Op};
use frame_support::{pallet_prelude::*, Blake2_128Concat};
use frame_system::pallet_prelude::*;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + scale_info::TypeInfo;
        type Balance: Parameter
            + Member
            + Copy
            + sp_runtime::traits::AtLeast32BitUnsigned
            + Default
            + MaxEncodedLen;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightData;
    }

    pub trait WeightData {
        fn set_paused() -> Weight;
        fn set_max_per_tx() -> Weight;
    }
    impl WeightData for () {
        fn set_paused() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn set_max_per_tx() -> Weight {
            Weight::from_parts(10_000, 0)
        }
    }

    #[pallet::storage]
    pub type Paused<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, bool, ValueQuery>;

    #[pallet::storage]
    pub type MaxPerTx<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, T::Balance, OptionQuery>;

    #[pallet::event]
    pub enum Event<T: Config> {
        Paused(T::AssetId, bool),
        MaxPerTxSet(T::AssetId, T::Balance),
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    impl<T: Config> AclProvider<T::AccountId, T::AssetId, T::Balance> for Pallet<T> {
        fn authorize(
            op: Op,
            ctx: &AclCtx<T::Balance, T::AccountId, T::AssetId>,
        ) -> Result<(), DispatchError> {
            // Pause gate for state-changing ops
            match op {
                Op::Mint
                | Op::Burn
                | Op::Transfer
                | Op::TransferFrom
                | Op::Shield
                | Op::Unshield => {
                    if Paused::<T>::get(ctx.asset) {
                        return Err(DispatchError::Other("ACL: paused"));
                    }
                }
                _ => {}
            }

            // Max-per-tx (only applies where amount matters)
            if let Some(max) = MaxPerTx::<T>::get(ctx.asset) {
                if ctx.amount > max {
                    return Err(DispatchError::Other("ACL: over per-tx limit"));
                }
            }
            Ok(())
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_paused())]
        pub fn set_paused(origin: OriginFor<T>, asset: T::AssetId, paused: bool) -> DispatchResult {
            // use EnsureRoot or a role-based origin (left out for brevity)
            ensure_root(origin)?;
            Paused::<T>::insert(asset, paused);
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_max_per_tx())]
        pub fn set_max_per_tx(
            origin: OriginFor<T>,
            asset: T::AssetId,
            max: T::Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            MaxPerTx::<T>::insert(asset, max);
            Ok(())
        }
    }
}
