#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod apis;
pub mod configs;
pub mod constants;
mod types;
mod weights;

use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::impl_opaque_keys;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
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
        // in Paseo, extrinsic base weight (smallest non-zero weight) is mapped to 1
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

#[cfg(not(feature = "tanssi"))]
impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
    }
}

#[cfg(feature = "tanssi")]
impl_opaque_keys! {
    pub struct SessionKeys { }
}

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    use crate::constants::VERSION;

    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

use openzeppelin_pallet_abstractions_proc::openzeppelin_construct_runtime;

#[cfg(feature = "tanssi")]
#[openzeppelin_construct_runtime]
mod runtime {
    struct System;

    struct XCM;

    struct Assets;

    struct Governance;

    struct Tanssi;
}

#[cfg(not(feature = "tanssi"))]
#[openzeppelin_construct_runtime]
mod runtime {
    struct System;

    struct XCM;

    struct Assets;

    struct Governance;

    struct Consensus;
}

#[cfg(test)]
mod test {
    use frame_support::weights::WeightToFeePolynomial;

    use crate::{
        constants::{POLY_DEGREE, VERSION},
        native_version, WeightToFee,
    };

    #[test]
    fn test_native_version() {
        let version = native_version();
        assert_eq!(version.runtime_version, VERSION);
    }

    #[test]
    fn test_weight_to_fee() {
        let mut fee = WeightToFee::polynomial();
        let coef = fee.pop().expect("no coef");
        assert!(!coef.negative);
        assert_eq!(coef.degree, POLY_DEGREE);
    }
}

#[cfg(test)]
mod test {
    use frame_support::weights::WeightToFeePolynomial;

    use crate::{
        constants::{POLY_DEGREE, VERSION},
        native_version, WeightToFee,
    };

    #[test]
    fn test_native_version() {
        let version = native_version();
        assert_eq!(version.runtime_version, VERSION);
    }

    #[test]
    fn test_weight_to_fee() {
        let mut fee = WeightToFee::polynomial();
        let coef = fee.pop().expect("no coef");
        assert!(!coef.negative);
        assert_eq!(coef.degree, POLY_DEGREE);
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmark;
