use frame_support::{
    pallet_prelude::*,
    storage::child,
};
use sp_core::storage::ChildInfo;
use sp_std::vec::Vec;

#[frame_support::pallet]
pub mod pallet_utxo {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub(super) type UtxoRoots<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, ChildInfo, OptionQuery>; // AssetId -> child trie

    // Simplified UTXO struct
    #[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
    pub struct Utxo {
        pub value: u128,
        pub owner: T::AccountId,
    }

    impl<T: Config> Pallet<T> {
        /// Generate a unique child trie key for an asset.
        fn asset_child_info(asset_id: u32) -> ChildInfo {
            let key = format!("utxo_child_{}", asset_id);
            ChildInfo::new_default(key.as_bytes())
        }

        /// Insert a new UTXO for given asset.
        pub fn insert_utxo(asset_id: u32, utxo_id: [u8; 32], utxo: &Utxo) {
            let child = Self::asset_child_info(asset_id);
            child::put(&child, &utxo_id, utxo);
        }

        /// Retrieve a UTXO if it exists.
        pub fn get_utxo(asset_id: u32, utxo_id: [u8; 32]) -> Option<Utxo> {
            let child = Self::asset_child_info(asset_id);
            child::get::<_, Utxo>(&child, &utxo_id)
        }

        /// Remove (spend) a UTXO.
        pub fn remove_utxo(asset_id: u32, utxo_id: [u8; 32]) {
            let child = Self::asset_child_info(asset_id);
            child::kill(&child, &utxo_id);
        }

        /// Wipe an entire asset’s UTXO set (e.g., delist asset).
        pub fn wipe_asset(asset_id: u32) {
            let child = Self::asset_child_info(asset_id);
            child::kill_prefix(&child, None);
        }
    }
}
