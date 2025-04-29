use frame_support::{dispatch::GetDispatchInfo, weights::Weight};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::{H160, H256};
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
    Xcm(xcm::v4::Location),
}
impl Default for AssetType {
    fn default() -> Self {
        Self::Xcm(xcm::v4::Location::here())
    }
}

impl From<xcm::v4::Location> for AssetType {
    fn from(location: xcm::v4::Location) -> Self {
        Self::Xcm(location)
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
            .call_weight
    }
}

/// This trait ensure we can convert AccountIds to AssetIds.
pub trait AccountIdAssetIdConversion<Account, AssetId> {
    // Get assetId and prefix from account
    fn account_to_asset_id(account: Account) -> Option<(Vec<u8>, AssetId)>;

    // Get AccountId from AssetId and prefix
    fn asset_id_to_account(prefix: &[u8], asset_id: AssetId) -> Account;
}

const FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8; 4];

// Instruct how to go from an H160 to an AssetID
// We just take the lowest 128 bits
impl AccountIdAssetIdConversion<AccountId, AssetId> for Runtime {
    /// The way to convert an account to assetId is by ensuring that the prefix is 0XFFFFFFFF
    /// and by taking the lowest 128 bits as the assetId
    fn account_to_asset_id(account: AccountId) -> Option<(Vec<u8>, AssetId)> {
        let h160_account: H160 = account.into();
        let mut data = [0u8; 16];
        let (prefix_part, id_part) = h160_account.as_fixed_bytes().split_at(4);
        if prefix_part == FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX {
            data.copy_from_slice(id_part);
            let asset_id: AssetId = u128::from_be_bytes(data);
            Some((prefix_part.to_vec(), asset_id))
        } else {
            None
        }
    }

    // The opposite conversion
    fn asset_id_to_account(prefix: &[u8], asset_id: AssetId) -> AccountId {
        let mut data = [0u8; 20];
        data[0..4].copy_from_slice(prefix);
        data[4..20].copy_from_slice(&asset_id.to_be_bytes());
        AccountId::from(data)
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
                    AssetManager::account_id(),
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
        use core::str::FromStr;

        use fp_account::AccountId20;

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
            data[0..4].copy_from_slice(FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX);
            data[4..20].copy_from_slice(&expected_asset_id.to_be_bytes());
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
            data[0..3].copy_from_slice(&FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX[0..3]);
            data[3..4].copy_from_slice(&[0]);
            data[4..20].copy_from_slice(&expected_asset_id.to_be_bytes());
            let account_id = AccountId::from(data);
            let res = Runtime::account_to_asset_id(account_id);
            assert_eq!(res, None);
        }

        #[test]
        fn test_asset_id_to_account() {
            let expected =
                AccountId20::from_str("0xFFFFFFFF00000000000000000000000000000001").unwrap();
            let asset_id = 1;
            let result =
                Runtime::asset_id_to_account(FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX, asset_id);
            assert_eq!(result, expected);
        }
    }

    mod asset_type {
        use crate::{configs::AssetType, types::AssetId};

        #[test]
        fn test_asset_type_default() {
            let default_asset_type = AssetType::default();
            match default_asset_type {
                AssetType::Xcm(location) => {
                    assert_eq!(location, xcm::v4::Location::here());
                }
            }
        }

        #[cfg(feature = "runtime-benchmarks")]
        #[test]
        fn test_asset_type_from_location_v3() {
            let location =
                xcm::v3::MultiLocation { parents: 1, interior: xcm::v3::Junctions::Here };

            // Use the From implementation directly
            let asset_type = AssetType::from(location);
            assert!(matches!(asset_type, AssetType::Xcm(_)));
        }

        #[test]
        fn test_asset_type_try_from_location_v4() {
            let location = xcm::v4::Location { parents: 1, interior: xcm::v4::Junctions::Here };
            let asset_type = AssetType::from(location.clone());
            match asset_type {
                AssetType::Xcm(loc) => assert_eq!(loc, location),
            }
        }

        #[test]
        fn test_asset_type_into_location() {
            let location = xcm::v4::Location::here();
            let asset_type = AssetType::Xcm(location.clone());
            let result: Option<xcm::v4::Location> = asset_type.into();
            assert_eq!(result, Some(location));
        }

        #[test]
        fn test_asset_type_into_asset_id() {
            let location = xcm::v4::Location { parents: 1, interior: xcm::v4::Junctions::Here };
            let asset_type = AssetType::Xcm(location);
            let asset_id: AssetId = asset_type.into();
            assert!(asset_id > 0);
        }

        #[cfg(feature = "runtime-benchmarks")]
        #[test]
        fn test_convert_v3_to_v4() {
            use std::sync::Arc;

            use xcm::v3::{Junction as JunctionV3, Junctions as JunctionsV3, MultiLocation};

            use crate::configs::convert_v3_to_v4;

            // Test with Here junctions
            let v3_location = MultiLocation { parents: 1, interior: JunctionsV3::Here };
            let v4_location = convert_v3_to_v4(v3_location).unwrap();
            assert_eq!(v4_location.parents, 1);
            assert_eq!(v4_location.interior, xcm::v4::Junctions::Here);

            // Test with X1 junction
            let v3_location = MultiLocation {
                parents: 0,
                interior: JunctionsV3::X1(JunctionV3::Parachain(1000)),
            };
            let v4_location = convert_v3_to_v4(v3_location).unwrap();
            assert_eq!(v4_location.parents, 0);

            // Create X1 junction with Arc
            let junction_array = Arc::new([xcm::v4::Junction::Parachain(1000)]);
            let expected_interior = xcm::v4::Junctions::X1(junction_array);
            assert_eq!(v4_location.interior, expected_interior);
        }

        #[cfg(feature = "runtime-benchmarks")]
        #[test]
        fn test_from_multilocation_for_asset_type() {
            use std::sync::Arc;

            use xcm::v3::{Junction as JunctionV3, Junctions as JunctionsV3, MultiLocation};

            // Test with valid conversion
            let v3_location = MultiLocation { parents: 1, interior: JunctionsV3::Here };
            let asset_type = AssetType::from(v3_location);
            assert!(matches!(asset_type, AssetType::Xcm(_)));

            // Test with more complex location
            let v3_location = MultiLocation {
                parents: 0,
                interior: JunctionsV3::X1(JunctionV3::Parachain(1000)),
            };
            let asset_type = AssetType::from(v3_location);
            match asset_type {
                AssetType::Xcm(location) => {
                    assert_eq!(location.parents, 0);

                    // Create X1 junction with Arc
                    let junction_array = Arc::new([xcm::v4::Junction::Parachain(1000)]);
                    let expected_interior = xcm::v4::Junctions::X1(junction_array);
                    assert_eq!(location.interior, expected_interior);
                }
            }
        }
    }
}
