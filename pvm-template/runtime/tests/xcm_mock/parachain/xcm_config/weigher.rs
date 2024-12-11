use frame_support::parameter_types;
use xcm::latest::prelude::*;
use xcm_builder::FixedWeightBounds;

use crate::xcm_mock::parachain::RuntimeCall;

parameter_types! {
    pub const UnitWeightCost: Weight = Weight::from_parts(1, 1);
    pub const MaxInstructions: u32 = 100;
}

pub type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
