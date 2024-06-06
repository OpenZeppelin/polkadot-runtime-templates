



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

impl Into<Option<xcm::v3::Location>> for AssetType {
	fn into(self) -> Option<xcm::v3::Location> {
		match self {
			Self::Xcm(location) => Some(location),
		}
	}
}

impl Into<Option<Location>> for AssetType {
	fn into(self) -> Option<Location> {
		match self {
			Self::Xcm(location) => xcm_builder::V4V3LocationConverter::convert_back(&location),
		}
	}
}

// Implementation on how to retrieve the AssetId from an AssetType
// We take it
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