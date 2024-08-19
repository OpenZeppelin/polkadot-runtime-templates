//! Primitive types used in runtime configuration

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Type used to represent an amount of native asset
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// Unit for timestamps: milliseconds since the unix epoch.
pub type Moment = u64;
