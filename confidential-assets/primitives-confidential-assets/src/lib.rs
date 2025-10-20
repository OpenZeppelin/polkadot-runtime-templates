//! Primitives shared between confidential assets pallets and precompiles
use frame_support::{pallet_prelude::*, BoundedVec};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::prelude::*;

/// Opaque ciphertext representing a "confidential" amount (analogous to `euint64`).
/// Keep bounded to protect PoV. Tune the size cap once your FHE/MW ciphertext is known.
pub type MaxCiphertextLen = ConstU32<128>;
pub type EncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

/// Opaque "external" encrypted amount (analogous to `externalEuint64`),
/// kept identical here (you may differentiate if your backend needs it).
pub type ExternalEncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

/// Proof/aux data blob used by the backend to validate "encrypted" transfers.
pub type MaxProofLen = ConstU32<8192>;
pub type InputProof = BoundedVec<u8, MaxProofLen>;

/// Optional data payload for `*_and_call` variants.
pub type MaxCallbackDataLen = ConstU32<4096>;
pub type CallbackData = BoundedVec<u8, MaxCallbackDataLen>;

// ------------------------------
// MW "raw tx" path (optional)
// ------------------------------

pub type Commitment = [u8; 32];

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct MwTransaction<AssetId> {
    pub asset_id: AssetId,
    pub input_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub output_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub excess_commitment: Commitment,
}

/// Abstract trait allowing access to UTXO storage.
///
/// Implemented by `pallet-multi-utxo`.
pub trait UtxoStorage<AssetId> {
    fn insert_commitment(asset: AssetId, commitment: Commitment);
    fn remove_commitment(asset: AssetId, commitment: &Commitment) -> bool;

    // NEW: Iterate all unspent outputs for a given asset.
    fn iter_unspent(asset: AssetId) -> Box<dyn Iterator<Item = Commitment>>;

    // NEW: Iterate unspent outputs *owned by* `who` (for ACL/balance views).
    // Ownership can be a simple index you maintain (commitment -> owner AccountId).
    // If you don't want to store owners, you can back this by an offchain indexer initially.
    fn iter_owned<AccountId: Clone>(
        asset: AssetId,
        who: &AccountId,
    ) -> Box<dyn Iterator<Item = Commitment>>;

    // Batch ops for performance:
    fn extend_commitments(asset: AssetId, commits: &[Commitment]);
    fn remove_commitments(asset: AssetId, commits: &[Commitment]) -> usize;
}

/// Optional raw MimbleWimble verification path in case you still submit whole MW txs.
pub trait MimbleWimbleCompute<AssetId> {
    fn verify_and_apply(tx: MwTransaction<AssetId>) -> DispatchResult;
}

// ------------------------------
// Backend abstraction
// ------------------------------

/// Backend that holds the **truth** for totals, balances, and executes transfers.
/// Implement this in your MW/FHE/UTXO layer (e.g., `pallet-mimblewimble` adapter).
pub trait ConfidentialBackend<AccountId, AssetId> {
    /// Returns the confidential total supply ciphertext for `asset`.
    fn total_supply(asset: AssetId) -> EncryptedAmount;

    /// Returns the confidential balance ciphertext for `who` under `asset`.
    /// Implementation may require a registered view key or ACL in the backend.
    fn balance_of(asset: AssetId, who: &AccountId) -> EncryptedAmount;

    /// Transfer using an **encrypted input** and a proof
    /// and return the encrypted amount actually transferred (may differ from input envelope).
    fn transfer_encrypted(
        asset: AssetId,
        encrypted_amount: ExternalEncryptedAmount,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    /// Transfer using an **ACL-authorized** encrypted amount (no input proof given here).
    /// Backend must enforce that from (or operator) is already authorized for `amount`.
    fn transfer_acl(
        asset: AssetId,
        amount: EncryptedAmount,
    ) -> Result<EncryptedAmount, DispatchError>;

    /// Optional: disclose a ciphertext to a plaintext amount if backend policy allows.
    fn disclose_amount(
        asset: AssetId,
        encrypted_amount: &EncryptedAmount,
        who: &AccountId,
    ) -> Result<u64, DispatchError>;
}

/// Metadata provider per asset (names, symbols, etc.).
/// Implement against your multi-asset registry or a separate pallet.
pub trait AssetMetadataProvider<AssetId> {
    fn name(asset: AssetId) -> Vec<u8>;
    fn symbol(asset: AssetId) -> Vec<u8>;
    fn decimals(asset: AssetId) -> u8;
    fn contract_uri(asset: AssetId) -> Vec<u8>;
}

/// Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived).
/// Default `()` does nothing.
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
