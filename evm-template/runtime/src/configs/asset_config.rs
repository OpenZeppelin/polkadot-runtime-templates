use frame_support::{dispatch::GetDispatchInfo, weights::Weight};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::traits::Hash as THash;
use sp_std::{
    convert::{From, Into},
    prelude::*,
};
use xcm::latest::Location;

use crate::{
    types::{AssetId, Balance},
    AssetManager, Assets, Runtime, RuntimeCall, RuntimeOrigin,
};

// Required for runtime benchmarks
pallet_assets::runtime_benchmarks_enabled! {
    pub struct BenchmarkHelper;
    impl<AssetIdParameter> pallet_assets::BenchmarkHelper<AssetIdParameter> for BenchmarkHelper
    where
        AssetIdParameter: From<u128>,
    {
        fn create_asset_id_parameter(id: u32) -> AssetIdParameter {
            (id as u128).into()
        }
    }
}

// Our AssetType. For now we only handle Xcm Assets
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum AssetType {
    Xcm(xcm::v3::Location),
}
impl Default for AssetType {
    fn default() -> Self {
        Self::Xcm(xcm::v3::Location::here())
    }
}

impl From<xcm::v3::Location> for AssetType {
    fn from(location: xcm::v3::Location) -> Self {
        Self::Xcm(location)
    }
}

// This can be removed once we fully adopt xcm::v4 everywhere
impl TryFrom<Location> for AssetType {
    type Error = ();

    fn try_from(location: Location) -> Result<Self, Self::Error> {
        Ok(Self::Xcm(location.try_into()?))
    }
}

impl From<AssetType> for Option<xcm::v3::Location> {
    fn from(val: AssetType) -> Self {
        match val {
            AssetType::Xcm(location) => Some(location),
        }
    }
}

// Implementation on how to retrieve the AssetId from an AssetType
impl From<AssetType> for AssetId {
    fn from(asset: AssetType) -> AssetId {
        match asset {
            AssetType::Xcm(id) => {
                let mut result: [u8; 16] = [0u8; 16];
                let hash: H256 = id.using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
                result.copy_from_slice(&hash.as_fixed_bytes()[0..16]);
                u128::from_le_bytes(result)
            }
        }
    }
}

#[derive(Clone, Default, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub struct AssetRegistrarMetadata {
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    pub decimals: u8,
    pub is_frozen: bool,
}

// We instruct how to register the Assets
// In this case, we tell it to Create an Asset in pallet-assets
pub struct AssetRegistrar;
use frame_support::{pallet_prelude::DispatchResult, transactional};

impl pallet_asset_manager::AssetRegistrar<Runtime> for AssetRegistrar {
    #[transactional]
    fn create_foreign_asset(
        asset: AssetId,
        min_balance: Balance,
        metadata: AssetRegistrarMetadata,
        is_sufficient: bool,
    ) -> DispatchResult {
        Assets::force_create(
            RuntimeOrigin::root(),
            asset.into(),
            AssetManager::account_id(),
            is_sufficient,
            min_balance,
        )?;

        // Lastly, the metadata
        Assets::force_set_metadata(
            RuntimeOrigin::root(),
            asset.into(),
            metadata.name,
            metadata.symbol,
            metadata.decimals,
            metadata.is_frozen,
        )
    }

    #[transactional]
    fn destroy_foreign_asset(asset: AssetId) -> DispatchResult {
        // Mark the asset as destroying
        Assets::start_destroy(RuntimeOrigin::root(), asset.into())
    }

    fn destroy_asset_dispatch_info_weight(asset: AssetId) -> Weight {
        // For us both of them (Foreign and Local) have the same annotated weight for a given
        // witness
        // We need to take the dispatch info from the destroy call, which is already annotated in
        // the assets pallet

        // This is the dispatch info of destroy
        RuntimeCall::Assets(pallet_assets::Call::<Runtime>::start_destroy { id: asset.into() })
            .get_dispatch_info()
            .weight
    }
}
