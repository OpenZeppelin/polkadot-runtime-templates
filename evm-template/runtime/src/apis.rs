use frame_support::{
    genesis_builder_helper::{build_config, create_default_config},
    traits::OnFinalize,
    weights::Weight,
};
use pallet_ethereum::{
    Call::transact, Transaction as EthereumTransaction, TransactionAction, TransactionData,
    TransactionStatus,
};
use pallet_evm::{Account as EVMAccount, FeeCalculator, Runner};
use polkadot_runtime_wrappers::impl_openzeppelin_evm_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160, H256, U256};
use sp_runtime::{
    traits::{Block as BlockT, Get, UniqueSaturatedInto},
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, Permill,
};
use sp_std::prelude::Vec;
use sp_version::RuntimeVersion;

#[cfg(not(feature = "async-backing"))]
use crate::Aura;
#[cfg(feature = "async-backing")]
use crate::{constants::SLOT_DURATION, types::ConsensusHook};
use crate::{
    constants::VERSION,
    types::{AccountId, Balance, Block, Executive, Nonce},
    Ethereum, InherentDataExt, ParachainSystem, Runtime, RuntimeCall, RuntimeGenesisConfig,
    SessionKeys, System, TransactionPayment, UncheckedExtrinsic,
};

impl_openzeppelin_evm_runtime_apis!();
