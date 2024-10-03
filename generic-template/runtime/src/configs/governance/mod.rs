//! OpenGov governance config

pub mod origins;
pub use origins::{Spender, WhitelistedCaller};
mod tracks;

use frame_support::{
    parameter_types,
    traits::{ConstU32, EitherOf},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess, EnsureSigned};

use crate::{
    constants::{
        currency::{CENTS, GRAND},
        DAYS,
    },
    types::{AccountId, Balance, BlockNumber},
    weights, Balances, Preimage, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Scheduler,
    Treasury,
};

parameter_types! {
    pub const MaxBalance: Balance = Balance::MAX;
}
pub type TreasurySpender = EitherOf<EnsureRootWithSuccess<AccountId, MaxBalance>, Spender>;

impl origins::pallet_custom_origins::Config for Runtime {}

parameter_types! {
    pub const AlarmInterval: BlockNumber = 1;
    pub const SubmissionDeposit: Balance = 3 * CENTS;
    pub const UndecidingTimeout: BlockNumber = 14 * DAYS;
}

impl pallet_referenda::Config for Runtime {
    type AlarmInterval = AlarmInterval;
    type CancelOrigin = EnsureRoot<AccountId>;
    type Currency = Balances;
    type KillOrigin = EnsureRoot<AccountId>;
    type MaxQueued = ConstU32<20>;
    type Preimages = Preimage;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Slash = Treasury;
    type SubmissionDeposit = SubmissionDeposit;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type Tally = pallet_conviction_voting::TallyOf<Runtime>;
    type Tracks = tracks::TracksInfo;
    type UndecidingTimeout = UndecidingTimeout;
    type Votes = pallet_conviction_voting::VotesOf<Runtime>;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_referenda::WeightInfo<Runtime>;
}
