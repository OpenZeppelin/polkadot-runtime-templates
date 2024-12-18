use pallet_xcm::XcmPassthrough;
use xcm_builder::{SignedAccountKey20AsNative, SovereignSignedViaLocation};

use crate::xcm_mock::parachain::{
    constants::RelayNetwork, location_converter::LocationConverter, RuntimeOrigin,
};

type XcmOriginToCallOrigin = (
    SovereignSignedViaLocation<LocationConverter, RuntimeOrigin>,
    SignedAccountKey20AsNative<RelayNetwork, RuntimeOrigin>,
    XcmPassthrough<RuntimeOrigin>,
);

pub type OriginConverter = XcmOriginToCallOrigin;
