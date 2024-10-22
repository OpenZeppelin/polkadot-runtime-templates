#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod apis;
pub mod configs;
pub mod constants;
mod precompiles;
pub use precompiles::OpenZeppelinPrecompiles;
mod types;
mod weights;

use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::H160;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
    impl_opaque_keys,
    traits::{DispatchInfoOf, Dispatchable, PostDispatchInfoOf},
    transaction_validity::{TransactionValidity, TransactionValidityError},
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;

use crate::{
    configs::pallet_custom_origins,
    constants::{currency::MILLICENTS, POLY_DEGREE, P_FACTOR, Q_FACTOR},
    weights::ExtrinsicBaseWeight,
};
pub use crate::{
    configs::RuntimeBlockWeights,
    types::{
        AccountId, Balance, Block, BlockNumber, Executive, Nonce, Signature, UncheckedExtrinsic,
    },
};

impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;

    fn is_self_contained(&self) -> bool {
        match self {
            RuntimeCall::Ethereum(call) => call.is_self_contained(),
            _ => false,
        }
    }

    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => call.check_self_contained(),
            _ => None,
        }
    }

    fn validate_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<TransactionValidity> {
        match self {
            RuntimeCall::Ethereum(call) => call.validate_self_contained(info, dispatch_info, len),
            _ => None,
        }
    }

    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) =>
                call.pre_dispatch_self_contained(info, dispatch_info, len),
            _ => None,
        }
    }

    fn apply_self_contained(
        self,
        info: Self::SignedInfo,
    ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) =>
                Some(call.dispatch(RuntimeOrigin::from(
                    pallet_ethereum::RawOrigin::EthereumTransaction(info),
                ))),
            _ => None,
        }
    }
}

/// Handles converting a weight scalar to a fee value, based on the scale and
/// granularity of the node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some
/// examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be
///     charged.
pub struct WeightToFee;

impl WeightToFeePolynomial for WeightToFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        // in Rococo, extrinsic base weight (smallest non-zero weight) is mapped to 1
        // MILLIUNIT: in our template, we map to 1/10 of that, or 1/10 MILLIUNIT
        let p = MILLICENTS / P_FACTOR;
        let q = Q_FACTOR * Balance::from(ExtrinsicBaseWeight::get().ref_time());
        smallvec![WeightToFeeCoefficient {
            degree: POLY_DEGREE,
            negative: false,
            coeff_frac: Perbill::from_rational(p % q, q),
            coeff_integer: p / q,
        }]
    }
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't
/// need to know the specifics of the runtime. They can then be made to be
/// agnostic over specific formats of data like extrinsics, allowing for them to
/// continue syncing the network through upgrades to even the core data
/// structures.
pub mod opaque {
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    use super::*;
    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
    }
}

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    use crate::constants::VERSION;

    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;
    #[runtime::pallet_index(1)]
    pub type ParachainSystem = cumulus_pallet_parachain_system;
    #[runtime::pallet_index(2)]
    pub type Timestamp = pallet_timestamp;
    #[runtime::pallet_index(3)]
    pub type ParachainInfo = parachain_info;
    #[runtime::pallet_index(4)]
    pub type Proxy = pallet_proxy;
    #[runtime::pallet_index(5)]
    pub type Utility = pallet_utility;
    #[runtime::pallet_index(6)]
    pub type Multisig = pallet_multisig;
    #[runtime::pallet_index(7)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(8)]
    pub type Preimage = pallet_preimage;

    // Monetary stuff.
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(13)]
    pub type Treasury = pallet_treasury;
    #[runtime::pallet_index(14)]
    pub type AssetManager = pallet_asset_manager;

    // Governance
    #[runtime::pallet_index(15)]
    pub type Sudo = pallet_sudo;
    #[runtime::pallet_index(16)]
    pub type ConvictionVoting = pallet_conviction_voting;
    #[runtime::pallet_index(17)]
    pub type Referenda = pallet_referenda;
    #[runtime::pallet_index(18)]
    pub type Origins = pallet_custom_origins;
    #[runtime::pallet_index(19)]
    pub type Whitelist = pallet_whitelist;

    // Collator support. The order of these 4 are important and shall not change.
    #[runtime::pallet_index(20)]
    pub type Authorship = pallet_authorship;
    #[runtime::pallet_index(21)]
    pub type CollatorSelection = pallet_collator_selection;
    #[runtime::pallet_index(22)]
    pub type Session = pallet_session;
    #[runtime::pallet_index(23)]
    pub type Aura = pallet_aura;
    #[runtime::pallet_index(24)]
    pub type AuraExt = cumulus_pallet_aura_ext;

    // XCM helpers.
    #[runtime::pallet_index(30)]
    pub type XcmpQueue = cumulus_pallet_xcmp_queue;
    #[runtime::pallet_index(31)]
    pub type PolkadotXcm = pallet_xcm;
    #[runtime::pallet_index(32)]
    pub type CumulusXcm = cumulus_pallet_xcm;
    #[runtime::pallet_index(33)]
    pub type MessageQueue = pallet_message_queue;
    #[runtime::pallet_index(34)]
    pub type XcmTransactor = pallet_xcm_transactor;

    // EVM
    #[runtime::pallet_index(40)]
    pub type Ethereum = pallet_ethereum;
    #[runtime::pallet_index(41)]
    pub type EVM = pallet_evm;
    #[runtime::pallet_index(42)]
    pub type BaseFee = pallet_base_fee;
    #[runtime::pallet_index(43)]
    pub type EVMChainId = pallet_evm_chain_id;
}

cumulus_pallet_parachain_system::register_validate_block! {
    Runtime = Runtime,
    BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmark;
