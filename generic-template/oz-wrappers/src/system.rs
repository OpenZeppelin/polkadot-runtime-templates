//! OZ-System Wrapper

use frame_support::{
    traits::{ConstU32, ConstU64, Get},
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

use crate::{constants::*, types::*};

/// Configuration exposed to the user
#[rustfmt::skip]
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

    // Remove and hardcode:
    type BlockWeight: Get<frame_support::pallet_prelude::Weight>;
    type BlockLength: Get<u32>;

    // Pallet Proxy Constants
    // - Required because pallet_proxy::DefaultConfig DNE && pallet_proxy::Config: frame_system::Config
    // - Could split into separate trait && impl
	type MaxProxies: Get<u32>;
	type MaxPending: Get<u32>;
    type ProxyDepositBase: Get<Balance>;
    type ProxyDepositFactor: Get<Balance>;
	type AnnouncementDepositBase: Get<Balance>;
	type AnnouncementDepositFactor: Get<Balance>;
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

    // Injected by Runtime:
    type OnSetCode = ();
    type BaseCallFilter = ();
    #[inject_runtime_type]
    type PalletInfo = ();
    #[inject_runtime_type]
    type RuntimeCall = ();
    #[inject_runtime_type]
    type RuntimeEvent = ();
    #[inject_runtime_type]
    type RuntimeOrigin = ();
    #[inject_runtime_type]
    type RuntimeTask = ();
    
    // Not assigned
    type PostInherents = ();
    type PostTransactions = ();
    type PreInherents = ();
    type SingleBlockMigrations = ();
    type SystemWeightInfo = ();
    type MultiBlockMigrator = ();
    type OnKilledAccount = ();
    type OnNewAccount = ();
}

#[rustfmt::skip]
impl<Runtime: OzSystemConfig> pallet_timestamp::DefaultConfig for OzSystem<Runtime> {
    // Used in Runtime
    #[cfg(feature = "experimental")]
    type MinimumPeriod = ConstU64<0>;
    #[cfg(not(feature = "experimental"))]
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type Moment = Moment;

    // Not assigned
    type OnTimestampSet = ();
    type WeightInfo = ();
}
