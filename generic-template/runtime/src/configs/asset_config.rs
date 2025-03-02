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
#[cfg(feature = "runtime-benchmarks")]
use xcm::v3::MultiLocation;

use crate::{
    types::{AccountId, AssetId, Balance},
    AssetManager, Assets, Runtime, RuntimeCall, RuntimeOrigin,
};

// Our AssetType. For now we only handle Xcm Assets
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum AssetType {
    Xcm(xcm::v4::Location),
}
impl Default for AssetType {
    fn default() -> Self {
        Self::Xcm(xcm::v4::Location::here())
    }
}

#[cfg(feature = "runtime-benchmarks")]
fn convert_v3_to_v4(v3: MultiLocation) -> Option<xcm::v4::Location> {
    Some(xcm::v4::Location {
        parents: v3.parents,
        interior: xcm::v4::Junctions::try_from(v3.interior).ok()?, // Returns None if conversion fails
    })
}

#[cfg(feature = "runtime-benchmarks")]
impl From<MultiLocation> for AssetType {
    fn from(value: MultiLocation) -> Self {
        Self::Xcm(convert_v3_to_v4(value).unwrap_or(xcm::v4::Location::default()))
    }
}

impl From<xcm::v4::Location> for AssetType {
    fn from(location: xcm::v4::Location) -> Self {
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

impl From<AssetType> for Option<xcm::v4::Location> {
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
                let mut result: [u8; 4] = [0u8; 4];
                let hash: H256 = id.using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
                result.copy_from_slice(&hash.as_fixed_bytes()[0..4]);
                u32::from_le_bytes(result)
            }
        }
    }
}

/// This trait ensure we can convert AccountIds to AssetIds.
pub trait AccountIdAssetIdConversion<Account, AssetId> {
    // Get assetId and prefix from account
    fn account_to_asset_id(account: Account) -> Option<(Vec<u8>, AssetId)>;

    // Get AccountId from AssetId and prefix
    fn asset_id_to_account(prefix: &[u8], asset_id: AssetId) -> Account;
}

const FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8; 28];

// Instruct how to go from an H256 to an AssetID
impl AccountIdAssetIdConversion<AccountId, AssetId> for Runtime {
    /// The way to convert an account to assetId is by ensuring that the prefix is 0XFFFFFFFF
    /// and by taking the lowest 128 bits as the assetId
    fn account_to_asset_id(account: AccountId) -> Option<(Vec<u8>, AssetId)> {
        let bytes: [u8; 32] = account.into();
        let h256_account: H256 = bytes.into();
        let mut data = [0u8; 4];
        let (prefix_part, id_part) = h256_account.as_fixed_bytes().split_at(28);
        if prefix_part == FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX {
            data.copy_from_slice(id_part);
            let asset_id: AssetId = u32::from_be_bytes(data);
            Some((prefix_part.to_vec(), asset_id))
        } else {
            None
        }
    }

    // The opposite conversion
    fn asset_id_to_account(prefix: &[u8], asset_id: AssetId) -> AccountId {
        let mut data = [0u8; 32];
        data[0..28].copy_from_slice(prefix);
        data[28..32].copy_from_slice(&asset_id.to_be_bytes());
        AccountId::from(data)
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
            sp_runtime::MultiAddress::Id(AssetManager::account_id()),
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
            .call_weight
    }
}

#[cfg(test)]
mod tests {
    mod asset_registrar {
        use pallet_asset_manager::AssetRegistrar;
        use sp_io::TestExternalities;

        use crate::{
            configs::{AssetRegistrar as Registrar, AssetRegistrarMetadata},
            types::AssetId,
            AssetManager, Assets, Runtime, RuntimeOrigin,
        };

        #[test]
        fn test_destroy_asset_dispatch_info_weight() {
            let asset_id: AssetId = 1;
            let _ = <Registrar as AssetRegistrar<Runtime>>::destroy_asset_dispatch_info_weight(
                asset_id,
            );
        }

        #[test]
        fn test_destroy_foreign_asset() {
            TestExternalities::default().execute_with(|| {
                let asset_id: AssetId = 1;
                let _ = Assets::force_create(
                    RuntimeOrigin::root(),
                    asset_id.into(),
                    sp_runtime::MultiAddress::Id(AssetManager::account_id()),
                    true,
                    1,
                );
                let res = <Registrar as AssetRegistrar<Runtime>>::destroy_foreign_asset(asset_id);
                assert!(res.is_ok());
            });
        }

        #[test]
        fn test_create_foreign_asset() {
            TestExternalities::default().execute_with(|| {
                let asset_id = 1;
                let res = <Registrar as AssetRegistrar<Runtime>>::create_foreign_asset(
                    asset_id,
                    1,
                    AssetRegistrarMetadata {
                        name: vec![0, 1, 2, 3],
                        symbol: vec![4, 5, 6, 7],
                        decimals: 6,
                        is_frozen: false,
                    },
                    false,
                );
                assert!(res.is_ok());
            });
        }
    }

    mod account_asset_id_conversion {
        use sp_runtime::AccountId32;

        use crate::{
            configs::{
                asset_config::FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX, AccountIdAssetIdConversion,
            },
            types::AssetId,
            AccountId, Runtime,
        };

        #[test]
        fn test_account_to_asset_id_success() {
            let expected_asset_id: AssetId = 1;
            let mut data = [0u8; 32];
            data[0..28].copy_from_slice(FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX);
            data[28..32].copy_from_slice(&expected_asset_id.to_be_bytes());
            let account_id = AccountId::from(data);
            let (prefix, asset_id) = Runtime::account_to_asset_id(account_id)
                .expect("Account to asset id conversion failed");
            assert_eq!(prefix, FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX);
            assert_eq!(asset_id, expected_asset_id);
        }

        #[test]
        fn test_account_to_asset_id_error() {
            let expected_asset_id: AssetId = 1;
            let mut data = [0u8; 32];
            data[0..27].copy_from_slice(&FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX[0..27]);
            data[27..28].copy_from_slice(&[0]);
            data[28..32].copy_from_slice(&expected_asset_id.to_be_bytes());
            let account_id = AccountId::from(data);
            let res = Runtime::account_to_asset_id(account_id);
            assert_eq!(res, None);
        }

        #[test]
        fn test_asset_id_to_account() {
            let expected = AccountId32::new([
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 1,
            ]);
            let asset_id = 1;
            let result =
                Runtime::asset_id_to_account(FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX, asset_id);
            assert_eq!(result, expected);
        }
    }

    mod asset_type {
        use std::sync::Arc;

        use crate::{configs::AssetType, types::AssetId};

        #[test]
        fn test_asset_type_default() {
            let default_asset_type = AssetType::default();
            assert_eq!(
                default_asset_type,
                AssetType::Xcm(xcm::v4::Location {
                    parents: 0,
                    interior: xcm::v4::Junctions::Here
                })
            );
        }

        #[test]
        fn test_asset_type_from_location_v3() {
            let location = xcm::v4::Location {
                parents: 0,
                interior: xcm::v4::Junctions::X1(Arc::new([xcm::v4::Junction::OnlyChild])),
            };
            let asset_type = AssetType::from(location.clone());

            assert_eq!(asset_type, AssetType::Xcm(location));
        }

        #[test]
        fn test_asset_type_try_from_location_v4() {
            let location =
                xcm::latest::Location { parents: 0, interior: xcm::latest::Junctions::Here };
            let old_location: xcm::v4::Location =
                xcm::v4::Location { parents: 0, interior: xcm::v4::Junctions::Here };
            let asset_type = AssetType::try_from(location)
                .expect("AssetType conversion from location v4 failed");

            assert_eq!(asset_type, AssetType::Xcm(old_location));
        }

        #[test]
        fn test_asset_type_into_location() {
            let location = xcm::v4::Location { parents: 0, interior: xcm::v4::Junctions::Here };
            let asset_type = AssetType::Xcm(location.clone());
            let converted: Option<xcm::v4::Location> = asset_type.into();
            assert_eq!(converted, Some(location));
        }

        #[test]
        fn test_asset_type_into_asset_id() {
            let location = xcm::v4::Location { parents: 0, interior: xcm::v4::Junctions::Here };
            let expected_asset_id: u32 = 3068126878;
            let asset_type = AssetType::Xcm(location);

            let asset_id = AssetId::from(asset_type);

            assert_eq!(asset_id, expected_asset_id);
        }
    }
}
