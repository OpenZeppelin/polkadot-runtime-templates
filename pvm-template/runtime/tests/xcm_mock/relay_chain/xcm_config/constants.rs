use frame_support::parameter_types;
use xcm::latest::prelude::*;

parameter_types! {
    pub TokensPerSecondPerByte: (AssetId, u128, u128) =
        (AssetId(TokenLocation::get()), 1_000_000_000_000, 1024 * 1024);
    pub const MaxAssetsIntoHolding: u32 = 64;
}

parameter_types! {
    pub const TokenLocation: Location = Here.into_location();
    pub RelayNetwork: NetworkId = ByGenesis([0; 32]);
    pub UniversalLocation: InteriorLocation = RelayNetwork::get().into();
    pub UnitWeightCost: u64 = 1_000;
}
