use polkadot_parachain_primitives::primitives::Id as ParaId;
use polkadot_runtime_parachains::origin;
use xcm_builder::{
    ChildParachainAsNative, ChildSystemParachainAsSuperuser, SignedAccountId32AsNative,
    SovereignSignedViaLocation,
};

use crate::xcm_mock::relay_chain::{
    constants::RelayNetwork, location_converter::LocationConverter, RuntimeOrigin,
};

type LocalOriginConverter = (
    SovereignSignedViaLocation<LocationConverter, RuntimeOrigin>,
    ChildParachainAsNative<origin::Origin, RuntimeOrigin>,
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    ChildSystemParachainAsSuperuser<ParaId, RuntimeOrigin>,
);

pub type OriginConverter = LocalOriginConverter;
