use frame_support::parameter_types;
use xcm::latest::prelude::*;
use xcm_builder::FixedWeightBounds;

use crate::xcm_mock::relay_chain::RuntimeCall;

parameter_types! {
    pub const BaseXcmWeight: Weight = Weight::from_parts(1_000, 1_000);
    pub const MaxInstructions: u32 = 100;
}

pub type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
