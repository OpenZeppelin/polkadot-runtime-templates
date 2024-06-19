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
    weights, Balances, Preimage, Referenda, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
    Scheduler, Treasury,
};

parameter_types! {
    pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_conviction_voting::Config for Runtime {
    type Currency = Balances;
    type MaxTurnout =
        frame_support::traits::tokens::currency::ActiveIssuanceOf<Balances, Self::AccountId>;
    type MaxVotes = ConstU32<512>;
    type Polls = Referenda;
    type RuntimeEvent = RuntimeEvent;
    type VoteLockingPeriod = VoteLockingPeriod;
    type WeightInfo = weights::pallet_conviction_voting::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MaxBalance: Balance = Balance::max_value();
}
pub type TreasurySpender = EitherOf<EnsureRootWithSuccess<AccountId, MaxBalance>, Spender>;

impl origins::pallet_custom_origins::Config for Runtime {}

impl pallet_whitelist::Config for Runtime {
    type DispatchWhitelistedOrigin = EitherOf<EnsureRoot<Self::AccountId>, WhitelistedCaller>;
    type Preimages = Preimage;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = weights::pallet_whitelist::WeightInfo<Runtime>;
    type WhitelistOrigin = EnsureRoot<Self::AccountId>;
}

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
    type WeightInfo = weights::pallet_referenda::WeightInfo<Runtime>;
}
