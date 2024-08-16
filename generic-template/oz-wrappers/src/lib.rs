#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{traits::Get, weights::constants::RocksDbWeight, Parameter};
use frame_support_procedural::{derive_impl, inject_runtime_type, register_default_impl};
pub use oz_config::OzSystemConfig;
use pallet_balances::AccountData;
use parity_scale_codec::{Decode, Encode, EncodeLike, FullCodec, MaxEncodedLen};
use primitives::{generic::*, Balance, BlockNumber, Hash, Moment};
use scale_info::{prelude::fmt::Debug, TypeInfo};
use sp_runtime::traits::{MaybeDisplay, MaybeSerializeDeserialize, Member};

#[derive(Clone, PartialEq, Eq)]
pub struct OzSystem<Runtime>(core::marker::PhantomData<Runtime>);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
#[register_default_impl(OzSystem)]
impl<Runtime: OzSystemConfig> frame_system::Config for OzSystem<Runtime> {
    // configured by OzSystemConfig
    type AccountData = AccountData<Balance>;
    type AccountId = <Runtime as OzSystemConfig>::AccountId;
    // TODO: replace with NormalFilter
    type BaseCallFilter = frame_support::traits::Everything;
    /// The block type.
    type Block = Block<Self::RuntimeCall>;
    // TODO: replace with polkadot_runtime_common::BlockHashCount
    type BlockHashCount = frame_support::traits::ConstU32<256>;
    // TODO: add these back
    // type BlockLength = ();
    // type BlockWeights = ();
    type DbWeight = RocksDbWeight;
    type Hash = Hash;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type Lookup = sp_runtime::traits::AccountIdLookup<Self::AccountId, ()>;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type Nonce = u32;
    // The action to take on a Runtime Upgrade
    // type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;

    // injected
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
    type SS58Prefix = <Runtime as OzSystemConfig>::SS58Prefix;
    type Version = <Runtime as OzSystemConfig>::Version;
}

#[frame_support::pallet]
pub mod oz_config {
    use frame_system::pallet_prelude::*;

    use super::*;

    /// Configurations exposed to the user
    /// OzSystem provides default config of frame_system::Config using this Config
    pub trait OzSystemConfig: 'static + Eq + Clone {
        type AccountId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + Ord
            + MaxEncodedLen;
        type SS58Prefix: Get<u16>;
        type Version: Get<sp_version::RuntimeVersion>;
        //type MinimumBalance: Get<Balance>;
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);
}
