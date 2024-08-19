#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{traits::{ConstU32, Get}, weights::constants::RocksDbWeight, Parameter};
use frame_support_procedural::{derive_impl, inject_runtime_type, register_default_impl};
use frame_system::limits::{BlockLength, BlockWeights};
pub use oz_config::OzSystemConfig;
use pallet_balances::AccountData;
use parity_scale_codec::{Decode, Encode, EncodeLike, FullCodec, MaxEncodedLen};
use polkadot_runtime_common::BlockHashCount;
use primitives::{generic::*, Balance, BlockNumber, Hash, Moment, Nonce};
use scale_info::{prelude::fmt::Debug, TypeInfo};
use sp_runtime::{
    traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
    Perbill,
};

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

#[register_default_impl(OzSystem)]
impl<Runtime: OzSystemConfig> frame_system::DefaultConfig for OzSystem<Runtime> {
    type AccountData = AccountData<Balance>;
    type AccountId = <Runtime as OzSystemConfig>::AccountId;
    // TODO: replace with NormalFilter
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockHashCount = BlockHashCount;
    type BlockLength = RuntimeBlockLength<Runtime>;
    type BlockWeights = RuntimeBlockWeights<Runtime>;
    type DbWeight = RocksDbWeight;
    type Hash = Hash;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type Lookup = sp_runtime::traits::AccountIdLookup<Self::AccountId, ()>;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type MultiBlockMigrator = ();
    type Nonce = Nonce;
    type OnKilledAccount = ();
    type OnNewAccount = ();
    type OnSetCode = ();
    #[inject_runtime_type]
    type PalletInfo = ();
    type PostInherents = ();
    type PostTransactions = ();
    type PreInherents = ();
    #[inject_runtime_type]
    type RuntimeCall = ();
    #[inject_runtime_type]
    type RuntimeEvent = ();
    #[inject_runtime_type]
    type RuntimeOrigin = ();
    #[inject_runtime_type]
    type RuntimeTask = ();
    type SS58Prefix = <Runtime as OzSystemConfig>::SS58Prefix;
    type SingleBlockMigrations = ();
    type SystemWeightInfo = ();
    type Version = <Runtime as OzSystemConfig>::Version;
}

#[frame_support::pallet]
pub mod oz_config {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use super::*;

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
        // TODO: remove and replace with constants?
        type BlockWeight: Get<Weight>;
        type BlockLength: Get<u32>;
        //type MinimumBalance: Get<Balance>;
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}
