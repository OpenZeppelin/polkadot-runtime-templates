#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame_support::traits::ConstU32;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::BoundedVec;

pub type ParamsId = u32; // e.g., 0 = TFHE_V1_RADIX_U64_4x16, define your table off-chain
pub type AlgoId = u16; // e.g., 1 = Zama-TFHE-Integer

// Reference to a server key kept off-chain, only tracks an ID/hash
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, MaxEncodedLen)]
pub struct ServerKeyRef {
    pub params: ParamsId,
    pub key_id: [u8; 32],
}

// Tight, POD-like blob for ciphertexts
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, TypeInfo, Debug, PartialEq, MaxEncodedLen)]
pub struct CtBlob {
    pub algo: AlgoId,
    pub params: ParamsId,
    pub payload: BoundedVec<u8, ConstU32<64>>, // serialized TFHE-rs ciphertext
}

// Public key blob for clients to encrypt amounts with it
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, TypeInfo, Debug, PartialEq, MaxEncodedLen)]
pub struct PubKeyBlob {
    pub params: ParamsId,
    pub payload: BoundedVec<u8, ConstU32<64>>,
}

// EVM log topics to announce work for the coprocessor.
pub mod topics {
    use hex_literal::hex;
    // put real keccak256 values here; single line hex only
    pub const FHE_ADD_REQ: [u8; 32] =
        hex!("4a09d57977f834e0f8dcb4a27c2ee356a3e6d4d0b2ecb9a9d2f0e1a8f0dbe6a7");
    pub const FHE_SUB_REQ: [u8; 32] =
        hex!("0f7a4a0f3b385d1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192");
    pub const FHE_CMP_REQ: [u8; 32] =
        hex!("9a8b7c6d5e4f30291827364556473829aa00bb11cc22dd33ee44ff5566778899");
}
