//! Primitive type definitions used in oz-wrapper and the runtime
#![cfg_attr(not(feature = "std"), no_std)]

pub mod generic;
// pub mod evm; TODO: add back once moved up

/// An index to a block.
pub type BlockNumber = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

pub type Balance = u128;

pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Nonce = u32;
