#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{traits::Get, Parameter};
use frame_support_procedural::{derive_impl, inject_runtime_type, register_default_impl};
pub use oz_config::Config;
use parity_scale_codec::{Decode, Encode, EncodeLike, FullCodec, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::MaybeSerializeDeserialize;

pub struct OzSystem<Runtime>(core::marker::PhantomData<Runtime>);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig)]
#[register_default_impl(OzSystem)]
impl<Runtime: Config> frame_system::Config for OzSystem<Runtime> {
    type AccountData = AccountData<Runtime::Balance>;
    type AccountId = Runtime::AccountId;
    // TODO: replace with other call filter used in runtime
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockHashCount = frame_support::traits::ConstU32<256>;
    type BlockLength = RuntimeBlockLength<Runtime>;
    type BlockWeights = RuntimeBlockWeights<Runtime>;
    type DbWeight = RocksDbWeight;
    type Hash = sp_core::hash::H256;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type Lookup = sp_runtime::traits::AccountIdLookup<Self::AccountId, ()>;
    type MaxConsumers = frame_support::traits::ConstU32<128>;
    type MultiBlockMigrator = ();
    type Nonce = u32;
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
    type SS58Prefix = u16;
    type SingleBlockMigrations = ();
    type SystemWeightInfo = ();
    //Runtime::Ss58;?
    type Version = Runtime::Version;
}

#[frame_support::pallet]
pub mod oz_config {
    use frame_system::pallet_prelude::*;

    use super::*;

    /// Configurations exposed to the user
    /// OzSystem provides default config of frame_system::Config using this Config
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The user account identifier type for the runtime.
        /// TODO: is there a way to keep the same trait bounds
        type AccountId: Parameter;
        type Balance: Parameter;
        // Parameter
        // + Member
        // + MaybeSerializeDeserialize
        // + Debug
        // + MaybeDisplay
        // + Ord
        // + MaxEncodedLen;
        // type BlockWeight: Get<Weight>;
        // type BlockTime: Get<types::Moment>;
        //type BlockLength: Get<u32>;
        // type MinimumBalance: Get<types::Balance>;
        //type Ss58: Get<u16>;
        type Version: Get<sp_version::RuntimeVersion>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}
