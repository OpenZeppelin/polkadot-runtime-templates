//! Fulfillment Hook + pallet-fhe-oracle types
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// Hook invoked when a request is fulfilled.
/// Your business pallet implements this to consume results.
pub trait OracleFulfillHook<T: frame_system::Config> {
    fn on_fulfill(id: u64, who: &T::AccountId, purpose: Purpose, payload: &[u8], result: &[u8]);
}

/// Default no-op implementation for runtimes with nothing hooked yet.
impl<T: frame_system::Config> OracleFulfillHook<T> for () {
    fn on_fulfill(_: u64, _: &T::AccountId, _: Purpose, _: &[u8], _: &[u8]) {}
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    Copy,
    Eq,
    PartialEq,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
)]
pub enum Purpose {
    /// e.g., reveal a transfer amount
    AmountDisclosure,
    /// free-form, you can version/tag externally
    Custom(u32),
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum Status {
    Pending,
    Completed,
    Failed,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Clone,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct Request<AccountId, B> {
    pub who: AccountId,
    pub purpose: Purpose,
    pub payload: B, // ciphertext bytes (or handles) bounded
    pub status: Status,
}
