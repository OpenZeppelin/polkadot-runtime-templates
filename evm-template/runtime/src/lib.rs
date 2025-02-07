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
        AccountId, AssetId, Balance, Block, BlockNumber, Executive, Nonce, Signature,
        UncheckedExtrinsic,
    },
};
#[cfg(feature = "runtime-benchmarks")]
use crate::{
    configs::{
        asset_config::AssetType, xcm_config::RelayLocation, FeeAssetId, TransactionByteFee,
        XcmExecutorConfig,
    },
    constants::currency::{CENTS, EXISTENTIAL_DEPOSIT},
    types::Address,
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

use openzeppelin_pallet_abstractions_proc::openzeppelin_construct_runtime;

#[cfg(not(feature = "tanssi"))]
#[openzeppelin_construct_runtime]
mod runtime {
    struct System;

    struct Consensus;

    struct XCM;

    struct Assets;

    struct Governance;

    struct EVM;
}

#[cfg(feature = "tanssi")]
#[openzeppelin_construct_runtime]
mod runtime {
    struct System;

    struct Tanssi;

    struct XCM;

    struct Assets;

    struct Governance;

    struct EVM;
}

use openzeppelin_pallet_abstractions_proc::openzeppelin_runtime_apis;

#[cfg(not(feature = "tanssi"))]
#[openzeppelin_runtime_apis]
mod apis {
    type Runtime = Runtime;
    type Block = Block;

    mod evm {
        type RuntimeCall = RuntimeCall;
        type Executive = Executive;
        type Ethereum = Ethereum;
    }

    mod assets {
        type RuntimeCall = RuntimeCall;
        type TransactionPayment = TransactionPayment;
        type Balance = Balance;
        type Oracle = Oracle;
        type OracleKey = AssetId;
    }

    mod consensus {
        type SessionKeys = SessionKeys;
        #[cfg(not(feature = "async-backing"))]
        type Aura = Aura;
        #[cfg(feature = "async-backing")]
        type SlotDuration = SLOT_DURATION;
        #[cfg(feature = "async-backing")]
        type ConsensusHook = ConsensusHook;
    }

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

    mod evm {
        type RuntimeCall = RuntimeCall;
        type Executive = Executive;
        type Ethereum = Ethereum;
        type Oracle = Oracle;
        type OracleKey = AssetId;
    }

    mod assets {
        type RuntimeCall = RuntimeCall;
        type TransactionPayment = TransactionPayment;
        type Balance = Balance;
    }

    mod tanssi {
        type SessionKeys = SessionKeys;
    }

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

    mod self_contained_call {
        mod is_self_contained {
            use core::str::FromStr;

            use ethereum::{LegacyTransaction, TransactionSignature};
            use fp_account::AccountId20;
            use fp_self_contained::SelfContainedCall;
            use frame_support::dispatch::GetDispatchInfo;
            use pallet_ethereum::TransactionAction;
            use sp_core::{H160, H256, U256};

            use crate::{Runtime, RuntimeCall, RuntimeOrigin};

            fn get_transaction() -> pallet_ethereum::Transaction {
                let mut input = vec![];
                let data = "095ea7b30000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d0000000000000000000000000000000000000000000001b1ae4d6e2ef5000000".as_bytes();
                input.extend_from_slice(data);
                pallet_ethereum::Transaction::Legacy(LegacyTransaction {
                    nonce: U256::from_dec_str("842").unwrap(),
                    gas_price: U256::from_dec_str("35540887252").unwrap(),
                    gas_limit: U256::from_dec_str("500000").unwrap(),
                    action: TransactionAction::Call(
                        H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                    ),
                    value: U256::from_dec_str("0").unwrap(),
                    input,
                    signature: TransactionSignature::new(
                        27,
                        H256::from_str(
                            "dd6a5b9e4f357728f7718589d802ec2317c73c2ee3c72deb51f07766f1294859",
                        )
                        .unwrap(),
                        H256::from_str(
                            "32d1883c43f8ed779219374be9e94174aa42fbf5ab63093f3fadd9e2aae0d1d1",
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                })
            }

            #[test]
            fn test_is_self_contained_ethereum() {
                let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                    transaction: get_transaction(),
                });
                assert!(call.is_self_contained())
            }

            #[test]
            fn test_is_not_self_contained() {
                let call = RuntimeCall::Balances(pallet_balances::Call::burn {
                    value: 1,
                    keep_alive: true,
                });
                assert!(!call.is_self_contained())
            }

            #[test]
            fn test_check_self_contained() {
                let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                    transaction: get_transaction(),
                });
                assert!(call.check_self_contained().is_some());
                assert!(call.check_self_contained().unwrap().is_ok());
            }

            #[test]
            fn test_check_not_self_contained() {
                let call = RuntimeCall::Balances(pallet_balances::Call::burn {
                    value: 1,
                    keep_alive: true,
                });

                assert!(call.check_self_contained().is_none());
            }

            #[test]
            fn test_validate_self_contained() {
                let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                    transaction: get_transaction(),
                });
                let info = call.get_dispatch_info();

                sp_io::TestExternalities::default().execute_with(|| {
                    let addr =
                        H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap();
                    let _ = pallet_balances::Pallet::<Runtime>::force_set_balance(
                        RuntimeOrigin::root(),
                        AccountId20(addr.0),
                        90000000000000000,
                    );
                    let i = call
                        .validate_self_contained(&addr, &info, 0)
                        .expect("wrong implementation")
                        .expect("wrong transaction");

                    assert_eq!(i.priority, 34540887252);
                });
            }

            #[test]
            fn test_validate_not_self_contained() {
                let call = RuntimeCall::Balances(pallet_balances::Call::burn {
                    value: 1,
                    keep_alive: true,
                });
                let info = call.get_dispatch_info();

                sp_io::TestExternalities::default().execute_with(|| {
                    let i = call.validate_self_contained(
                        &H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap(),
                        &info,
                        0,
                    );

                    assert!(i.is_none());
                });
            }

            #[test]
            fn test_pre_dispatch_self_contained() {
                let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                    transaction: get_transaction(),
                });
                let info = call.get_dispatch_info();

                sp_io::TestExternalities::default().execute_with(|| {
                    let addr =
                        H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap();
                    let who = AccountId20(addr.0);
                    let _ = pallet_balances::Pallet::<Runtime>::force_set_balance(
                        RuntimeOrigin::root(),
                        who,
                        90000000000000000,
                    );
                    // I do not know any other way to increase nonce
                    for _ in 0..842 {
                        frame_system::Pallet::<Runtime>::inc_account_nonce(who);
                    }
                    let () = call
                        .pre_dispatch_self_contained(&addr, &info, 0)
                        .expect("wrong implementation")
                        .expect("wrong transaction");
                });
            }

            #[test]
            fn test_pre_dispatch_not_self_contained() {
                let call = RuntimeCall::Balances(pallet_balances::Call::burn {
                    value: 1,
                    keep_alive: true,
                });
                let info = call.get_dispatch_info();

                sp_io::TestExternalities::default().execute_with(|| {
                    let i = call.pre_dispatch_self_contained(
                        &H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap(),
                        &info,
                        0,
                    );

                    assert!(i.is_none());
                });
            }

            #[test]
            fn test_apply_self_contained() {
                let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                    transaction: get_transaction(),
                });

                sp_io::TestExternalities::default().execute_with(|| {
                    let addr =
                        H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap();
                    let who = AccountId20(addr.0);
                    let _ = pallet_balances::Pallet::<Runtime>::force_set_balance(
                        RuntimeOrigin::root(),
                        who,
                        90000000000000000,
                    );
                    // I do not know any other way to increase nonce
                    for _ in 0..842 {
                        frame_system::Pallet::<Runtime>::inc_account_nonce(who);
                    }
                    let _ = call
                        .apply_self_contained(addr)
                        .expect("wrong implementation")
                        .expect("wrong transaction");
                });
            }

            #[test]
            fn test_apply_not_self_contained() {
                let call = RuntimeCall::Balances(pallet_balances::Call::burn {
                    value: 1,
                    keep_alive: true,
                });

                sp_io::TestExternalities::default().execute_with(|| {
                    let i = call.apply_self_contained(
                        H160::from_str("0x78DFFE34196A5987fb73fb9bbfd55a2A33e467Fb").unwrap(),
                    );

                    assert!(i.is_none());
                });
            }
        }
    }
}
