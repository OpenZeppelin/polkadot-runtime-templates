use frame_support::{
    dispatch::GetDispatchInfo,
    parameter_types,
    traits::AsEnsureOriginWithArg,
    weights::{ConstantMultiplier, Weight},
};
use frame_system::{EnsureRoot, EnsureSigned};
use openzeppelin_polkadot_wrappers::{impl_openzeppelin_assets, AssetsConfig};
use parity_scale_codec::{Decode, Encode};
use polkadot_runtime_common::SlowAdjustingFeeUpdate;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::traits::Hash as THash;
use sp_std::{
    convert::{From, Into},
    prelude::*,
};
use xcm::latest::Location;

use crate::{
    configs::OpenZeppelinRuntime,
    constants::currency::{deposit, CENTS, EXISTENTIAL_DEPOSIT, MICROCENTS},
    types::{AccountId, AssetId, Balance},
    weights, AssetManager, Assets, Balances, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
    WeightToFee,
};

parameter_types! {
    pub const AssetDeposit: Balance = 10 * CENTS;
    pub const AssetAccountDeposit: Balance = deposit(1, 16);
    pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
impl AssetsConfig for OpenZeppelinRuntime {
    type ApprovalDeposit = ApprovalDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type AssetDeposit = AssetDeposit;
    type AssetId = AssetId;
    type AssetRegistrar = AssetRegistrar;
    type AssetRegistrarMetadata = AssetRegistrarMetadata;
    type AssetType = AssetType;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type ForeignAssetModifierOrigin = EnsureRoot<AccountId>;
    type WeightToFee = WeightToFee;
}
impl_openzeppelin_assets!(OpenZeppelinRuntime);

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
            .weight
    }
}
