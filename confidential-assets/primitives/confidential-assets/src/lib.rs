//! Types and traits for confidential assets crates
use frame_support::{pallet_prelude::*, BoundedVec};
use sp_std::prelude::*;

/// Opaque ciphertext representing a "confidential" amount.
pub type MaxCiphertextLen = ConstU32<128>;
pub type EncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

/// Opaque "external" encrypted amount (e.g., envelope sent with proof).
pub type ExternalEncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

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
    // --- Key management (Solana/Zether-like) ---
    fn set_public_key(who: &AccountId, elgamal_pk: &PublicKeyBytes) -> Result<(), DispatchError>;

    // --- Read helpers ---
    fn total_supply(asset: AssetId) -> EncryptedAmount;
    fn balance_of(asset: AssetId, who: &AccountId) -> EncryptedAmount;

    // --- Core ops ---
    fn transfer_encrypted(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        encrypted_amount: ExternalEncryptedAmount,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    fn transfer_acl(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        amount: EncryptedAmount,
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

    /// Verify a transfer and **compute the new opaque per-account ciphertexts**.
    /// Requires receiver to call `pallet_zk_elgamal::accept_pending` to move from
    /// pending to available confidential balance for sending which internally calls
    /// `verify_transfer_to_receiver`.
    fn verify_transfer_sent(
        asset: &[u8],
        from_pk: &[u8],
        to_pk: &[u8],
        from_old: &[u8],
        to_old: &[u8],
        delta_ct: &[u8],
        proof: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error>;
    // returns (from_new_balance_cipher, to_new_balance_cipher)

    /// Verify transfer received to update receiver available balance
    fn verify_transfer_received(
        asset: &[u8],
        from_pk_bytes: &[u8],
        from_old_avail_bytes: &[u8],
        from_old_pending_bytes: &[u8],
        deposit_amounts: &[u8],
        deposits_sum: &[u8],
        proof: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error>;
    // returns (from_new_available_balance_cipher, from_new_pending_balance_cipher)

    /// ACL transfer: same as above but without proof. Implementation must enforce policy.
    fn acl_transfer_sent(
        asset: &[u8],
        from_pk: &[u8],
        to_pk: &[u8],
        from_old: &[u8],
        to_old: &[u8],
        amount_cipher: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error>;
    // returns (from_new_balance_cipher, to_new_balance_cipher)

    /// Optional disclosure (policy dependent). Return plaintext amount or error.
    fn disclose(asset: &[u8], who_pk: &[u8], cipher: &[u8]) -> Result<u64, Self::Error>;
}
