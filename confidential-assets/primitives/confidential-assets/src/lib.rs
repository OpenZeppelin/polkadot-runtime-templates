//! Types and traits for confidential assets crates
use frame_support::{pallet_prelude::*, BoundedVec};
use sp_std::prelude::*;

/// ZK El Gamal Ciphertext
/// bytes 0..32 = Pederson commitment
/// bytes 33..64 = El Gamal decrypt handle
pub type EncryptedAmount = [u8; 64];
/// Commitment type (compressed Ristretto)
pub type Commitment = [u8; 32];

/// Proof/aux data blob used by the backend to validate encrypted transfers.
pub type MaxProofLen = ConstU32<8192>;
pub type InputProof = BoundedVec<u8, MaxProofLen>;

/// Optional data payload for `*_and_call` variants.
pub type MaxCallbackDataLen = ConstU32<4096>;
pub type CallbackData = BoundedVec<u8, MaxCallbackDataLen>;

/// Zether/Solana-style public key bytes (ElGamal or similar).
pub type MaxPubKeyLen = ConstU32<64>;
pub type PublicKeyBytes = BoundedVec<u8, MaxPubKeyLen>;

/// Backend that holds the **truth** for totals, balances, public keys, and executes transfers.
pub trait ConfidentialBackend<AccountId, AssetId> {
    // --- Key management ---
    fn set_public_key(who: &AccountId, elgamal_pk: &PublicKeyBytes) -> Result<(), DispatchError>;

    // --- Read helpers ---
    fn total_supply(asset: AssetId) -> Commitment;
    fn balance_of(asset: AssetId, who: &AccountId) -> Commitment;

    // --- Core ops ---
    fn transfer_encrypted(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        encrypted_amount: EncryptedAmount,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    // --- Optional (policy-dependent) ---
    fn disclose_amount(
        asset: AssetId,
        encrypted_amount: &EncryptedAmount,
        who: &AccountId,
    ) -> Result<u64, DispatchError>;
}

/// Metadata provider per asset (names, symbols, etc.).
pub trait AssetMetadataProvider<AssetId> {
    fn name(asset: AssetId) -> Vec<u8>;
    fn symbol(asset: AssetId) -> Vec<u8>;
    fn decimals(asset: AssetId) -> u8;
    fn contract_uri(asset: AssetId) -> Vec<u8>;
}

/// Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived).
pub trait OnConfidentialTransfer<AccountId, AssetId> {
    /// Called after successful transfer if the extrinsic is an `*_and_call` variant.
    /// Return `Ok(())` to accept; `Err` to revert (pallet will bubble up error).
    fn on_confidential_transfer_received(
        from: &AccountId,
        to: &AccountId,
        asset: &AssetId,
        transferred: &EncryptedAmount,
        data: &CallbackData,
    ) -> Result<(), DispatchError>;
}
impl<AccountId, AssetId> OnConfidentialTransfer<AccountId, AssetId> for () {
    fn on_confidential_transfer_received(
        _from: &AccountId,
        _to: &AccountId,
        _asset: &AssetId,
        _transferred: &EncryptedAmount,
        _data: &CallbackData,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Abstract verifier boundary. Implement in the runtime.
///
// TODO:
// - verify_{mint, burn}_{to_send, received}
pub trait ZkVerifier {
    type Error;

    /// Sender phase: verify link/range (as implemented) and compute new commitments.
    /// Inputs:
    /// - `from_old_avail_commit`, `to_old_pending_commit`: 0 or 32 bytes
    /// - `delta_ct`: 64B ElGamal ciphertext (C||D)
    /// - `proof_bundle`: sender bundle bytes
    ///
    /// Returns:
    /// - (from_new_available_commit, to_new_pending_commit), both 32B
    fn verify_transfer_sent(
        asset: &[u8],
        from_pk: &[u8],
        to_pk: &[u8],
        from_old_avail_commit: &[u8], // empty => identity
        to_old_pending_commit: &[u8], // empty => identity
        delta_ct: &[u8],              // 64B
        proof_bundle: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error>;

    /// Receiver phase (Option A): accept selected UTXO deposits.
    /// Inputs:
    /// - `avail_old_commit`, `pending_old_commit`: 0 or 32 bytes
    /// - `pending_commits`: slice of 32B commitments for the consumed UTXOs (Σ must equal ΔC)
    /// - `accept_envelope`: delta_comm(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
    ///
    /// Returns:
    /// - (avail_new_commit, pending_new_commit), both 32B
    fn verify_transfer_received(
        asset: &[u8],
        who_pk: &[u8],
        avail_old_commit: &[u8],      // empty => identity
        pending_old_commit: &[u8],    // empty => identity
        pending_commits: &[[u8; 32]], // UTXO C’s to sum
        accept_envelope: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error>;

    /// Optional disclosure
    fn disclose(asset: &[u8], who_pk: &[u8], cipher: &[u8]) -> Result<u64, Self::Error>;
}

pub trait OperatorRegistry<AccountId, AssetId, BlockNumber> {
    /// Return true if `operator` is currently authorized to operate for (`holder`, `asset`) at `now`.
    fn is_operator(
        holder: &AccountId,
        asset: &AssetId,
        operator: &AccountId,
        now: BlockNumber,
    ) -> bool;
}

impl<AccountId, AssetId, BlockNumber> OperatorRegistry<AccountId, AssetId, BlockNumber> for () {
    fn is_operator(
        _holder: &AccountId,
        _asset: &AssetId,
        _operator: &AccountId,
        _now: BlockNumber,
    ) -> bool {
        false
    }
}
