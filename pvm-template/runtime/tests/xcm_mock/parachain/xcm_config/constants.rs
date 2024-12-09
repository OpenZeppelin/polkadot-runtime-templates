use frame_support::parameter_types;
use xcm::latest::prelude::*;
use xcm_simulator::mock_message_queue::ParachainId;

use crate::xcm_mock::parachain::Runtime;

parameter_types! {
    pub KsmPerSecondPerByte: (AssetId, u128, u128) = (AssetId(Parent.into()), 1, 1);
    pub const MaxAssetsIntoHolding: u32 = 64;
}

parameter_types! {
    pub const KsmLocation: Location = Location::parent();
    pub const RelayNetwork: NetworkId = NetworkId::Kusama;
    pub UniversalLocation: InteriorLocation = [GlobalConsensus(RelayNetwork::get()), Parachain(ParachainId::<Runtime>::get().into())].into();
}
