#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod configs;
pub mod constants;
mod precompiles;
pub use precompiles::OpenZeppelinPrecompiles;
mod types;
mod weights;

use frame_support::{
    traits::OnFinalize,
    weights::{WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial},
};
use smallvec::smallvec;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::H160;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
    impl_opaque_keys,
    traits::{DispatchInfoOf, Dispatchable, Get, PostDispatchInfoOf, UniqueSaturatedInto},
    transaction_validity::{TransactionValidity, TransactionValidityError},
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use sp_std::prelude::{Vec, *};
#[cfg(feature = "std")]
use sp_version::NativeVersion;

use crate::{
    configs::pallet_custom_origins,
    constants::{currency::MILLICENTS, POLY_DEGREE, P_FACTOR, Q_FACTOR, VERSION},
    weights::ExtrinsicBaseWeight,
};
pub use crate::{
    configs::RuntimeBlockWeights,
    types::{
        AccountId, Balance, Block, BlockNumber, Executive, Nonce, Signature, UncheckedExtrinsic,
    },
};
#[cfg(feature = "runtime-benchmarks")]
use crate::{
    configs::{
        asset_config::AssetType, xcm_config::RelayLocation, FeeAssetId, TransactionByteFee,
        XcmExecutorConfig,
    },
    constants::currency::{CENTS, EXISTENTIAL_DEPOSIT},
    types::{Address, AssetId},
};
#[cfg(feature = "async-backing")]
use crate::{constants::SLOT_DURATION, types::ConsensusHook};

#[cfg(feature = "runtime-benchmarks")]
type ExistentialDeposit = sp_core::ConstU128<EXISTENTIAL_DEPOSIT>;

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
    pub struct SessionKeys {}
}

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    use crate::constants::VERSION;

    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

use openzeppelin_polkadot_wrappers_proc::openzeppelin_construct_runtime;

#[cfg(not(feature = "tanssi"))]
#[openzeppelin_construct_runtime]
mod runtime {
    #[abstraction]
    struct System;

    #[abstraction]
    struct Consensus;

    #[abstraction]
    struct XCM;

    #[abstraction]
    struct Assets;

    #[abstraction]
    struct Governance;

    #[abstraction]
    struct EVM;
}

#[cfg(feature = "tanssi")]
#[openzeppelin_construct_runtime]
mod runtime {
    #[abstraction]
    struct System;

    #[abstraction]
    struct Tanssi;

    #[abstraction]
    struct XCM;

    #[abstraction]
    struct Assets;

    #[abstraction]
    struct Governance;

    #[abstraction]
    struct EVM;
}

use openzeppelin_polkadot_wrappers_proc::openzeppelin_runtime_apis;

#[cfg(not(feature = "tanssi"))]
#[openzeppelin_runtime_apis]
mod apis {
    type Runtime = Runtime;
    type Block = Block;

    #[abstraction]
    mod evm {
        type RuntimeCall = RuntimeCall;
        type Executive = Executive;
        type Ethereum = Ethereum;
    }

    #[abstraction]
    mod assets {
        type RuntimeCall = RuntimeCall;
        type TransactionPayment = TransactionPayment;
        type Balance = Balance;
    }

    #[abstraction]
    mod consensus {
        type SessionKeys = SessionKeys;
        #[cfg(not(feature = "async-backing"))]
        type Aura = Aura;
        #[cfg(feature = "async-backing")]
        type SlotDuration = SLOT_DURATION;
        #[cfg(feature = "async-backing")]
        type ConsensusHook = ConsensusHook;
    }

    #[abstraction]
    mod system {
        type Executive = Executive;
        type System = System;
        type ParachainSystem = ParachainSystem;
        type RuntimeVersion = VERSION;
        type AccountId = AccountId;
        type Nonce = Nonce;
        type RuntimeGenesisConfig = RuntimeGenesisConfig;
        type RuntimeBlockWeights = RuntimeBlockWeights;
    }

    #[abstraction]
    mod benchmarks {
        type AllPalletsWithSystem = AllPalletsWithSystem;
        type Assets = Assets;
        type AssetManager = AssetManager;
        type AssetType = AssetType;
        type RuntimeOrigin = RuntimeOrigin;
        type RelayLocation = RelayLocation;
        type ParachainSystem = ParachainSystem;
        type System = System;
        type ExistentialDeposit = ExistentialDeposit;
        type AssetId = AssetId;
        type XCMConfig = XcmExecutorConfig;
        type AccountId = AccountId;
        type Cents = CENTS;
        type FeeAssetId = FeeAssetId;
        type TransactionByteFee = TransactionByteFee;
        type Address = Address;
        type Balances = Balances;
    }
}

#[cfg(feature = "tanssi")]
#[openzeppelin_runtime_apis]
mod apis {
    type Runtime = Runtime;
    type Block = Block;

    #[abstraction]
    mod evm {
        type RuntimeCall = RuntimeCall;
        type Executive = Executive;
        type Ethereum = Ethereum;
    }

    #[abstraction]
    mod assets {
        type RuntimeCall = RuntimeCall;
        type TransactionPayment = TransactionPayment;
        type Balance = Balance;
    }

    #[abstraction]
    mod tanssi {
        type SessionKeys = SessionKeys;
    }

    #[abstraction]
    mod system {
        type Executive = Executive;
        type System = System;
        type ParachainSystem = ParachainSystem;
        type RuntimeVersion = VERSION;
        type AccountId = AccountId;
        type Nonce = Nonce;
        type RuntimeGenesisConfig = RuntimeGenesisConfig;
        type RuntimeBlockWeights = RuntimeBlockWeights;
    }

    #[abstraction]
    mod benchmarks {
        type AllPalletsWithSystem = AllPalletsWithSystem;
        type Assets = Assets;
        type AssetManager = AssetManager;
        type AssetType = AssetType;
        type RuntimeOrigin = RuntimeOrigin;
        type RelayLocation = RelayLocation;
        type ParachainSystem = ParachainSystem;
        type System = System;
        type ExistentialDeposit = ExistentialDeposit;
        type AssetId = AssetId;
        type XCMConfig = XcmExecutorConfig;
        type AccountId = AccountId;
        type Cents = CENTS;
        type FeeAssetId = FeeAssetId;
        type TransactionByteFee = TransactionByteFee;
        type Address = Address;
        type Balances = Balances;
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmark;
