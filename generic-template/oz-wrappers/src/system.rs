//! OZ-System Wrapper

use frame_support::{
    traits::{ConstU32, Everything, Get},
    weights::constants::RocksDbWeight,
    Parameter,
};
use frame_support_procedural::{inject_runtime_type, register_default_impl};
use frame_system::limits::{BlockLength, BlockWeights};
use pallet_balances::AccountData;
use parity_scale_codec::MaxEncodedLen;
use polkadot_runtime_common::BlockHashCount;
use scale_info::prelude::fmt::Debug;
use sp_runtime::{
    traits::{AccountIdLookup, BlakeTwo256, MaybeDisplay, MaybeSerializeDeserialize, Member},
    Perbill,
};

use crate::types::*;

/// Configurations exposed to the user
/// OzSystem provides default config of frame_system::Config using this Config
pub trait OzSystemConfig {
    type AccountId: Parameter
        + Member
        + MaybeSerializeDeserialize
        + Debug
        + MaybeDisplay
        + Ord
        + MaxEncodedLen;
    type SS58Prefix: Get<u16>;
    type Version: Get<sp_version::RuntimeVersion>;
    // TODO: remove and hardcode
    type BlockWeight: Get<frame_support::pallet_prelude::Weight>;
    // TODO: remove and hardcode
    type BlockLength: Get<u32>;
}

pub struct OzSystem<Runtime>(core::marker::PhantomData<Runtime>);

pub struct RuntimeBlockWeights<Runtime: OzSystemConfig>(core::marker::PhantomData<Runtime>);
impl<Runtime: OzSystemConfig> Get<BlockWeights> for RuntimeBlockWeights<Runtime> {
    fn get() -> BlockWeights {
        BlockWeights::with_sensible_defaults(Runtime::BlockWeight::get(), Perbill::one())
    }
}

pub struct RuntimeBlockLength<Runtime: OzSystemConfig>(core::marker::PhantomData<Runtime>);
impl<Runtime: OzSystemConfig> Get<BlockLength> for RuntimeBlockLength<Runtime> {
    fn get() -> BlockLength {
        BlockLength::max(Runtime::BlockLength::get())
    }
}

#[rustfmt::skip]
#[register_default_impl(OzSystem)]
impl<Runtime: OzSystemConfig> frame_system::DefaultConfig for OzSystem<Runtime> {
    // Used in Runtime:
    type AccountId = <Runtime as OzSystemConfig>::AccountId;
    type SS58Prefix = <Runtime as OzSystemConfig>::SS58Prefix;
    type Version = <Runtime as OzSystemConfig>::Version;
    type AccountData = AccountData<Balance>;
    type BlockHashCount = BlockHashCount;
    type BlockLength = RuntimeBlockLength<Runtime>;
    type BlockWeights = RuntimeBlockWeights<Runtime>;
    type DbWeight = RocksDbWeight;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type Lookup = AccountIdLookup<Self::AccountId, ()>;
    type Nonce = Nonce;
    type MaxConsumers = ConstU32<16>;

    // Overwritten in Runtime:
    type OnSetCode = ();
    type BaseCallFilter = Everything;

    // Injected by Runtime:
    #[inject_runtime_type]
    type PalletInfo = ();
    #[inject_runtime_type]
    type RuntimeCall = ();
    #[inject_runtime_type]
    type RuntimeEvent = ();
    #[inject_runtime_type]
    type RuntimeOrigin = ();
    #[inject_runtime_type]

    // NOT assigned:
    type RuntimeTask = ();
    type PostInherents = ();
    type PostTransactions = ();
    type PreInherents = ();
    type SingleBlockMigrations = ();
    type SystemWeightInfo = ();
    type MultiBlockMigrator = ();
    type OnKilledAccount = ();
    type OnNewAccount = ();
}
