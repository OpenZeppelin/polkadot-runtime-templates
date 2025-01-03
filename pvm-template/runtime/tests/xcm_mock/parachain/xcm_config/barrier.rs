use frame_support::traits::Everything;
use xcm_builder::AllowUnpaidExecutionFrom;

pub type Barrier = AllowUnpaidExecutionFrom<Everything>;
