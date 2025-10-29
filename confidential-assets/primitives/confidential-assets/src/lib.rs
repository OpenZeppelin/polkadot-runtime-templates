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
pub trait ConfidentialBackend<AccountId, AssetId, Balance> {
    fn set_public_key(who: &AccountId, elgamal_pk: &PublicKeyBytes) -> Result<(), DispatchError>;

    // Read encrypted balances state
    fn total_supply(asset: AssetId) -> Commitment;
    fn balance_of(asset: AssetId, who: &AccountId) -> Commitment;

    fn disclose_amount(
        asset: AssetId,
        encrypted_amount: &EncryptedAmount,
        who: &AccountId,
    ) -> Result<Balance, DispatchError>;

    fn transfer_encrypted(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        encrypted_amount: EncryptedAmount,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    fn claim_encrypted(
        asset: AssetId,
        from: &AccountId,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    fn mint_encrypted(
        asset: AssetId,
        to: &AccountId,
        amount: Balance,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    fn burn_encrypted(
        asset: AssetId,
        from: &AccountId,
        amount: EncryptedAmount,
        input_proof: InputProof,
    ) -> Result<Balance, DispatchError>;
}

/// Off/On-ramp for the public side of an asset.
/// Semantics:
/// - `burn` = move *public* funds from `who` into the pallet's custody
///             (so we can mint confidential).
/// - `mint` = move *public* funds from the pallet's custody out to `who`
///             (after we burn/debit confidential).
pub trait Ramp<AccountId, AssetId, Amount> {
    type Error;

    fn burn(who: &AccountId, asset: &AssetId, amount: Amount) -> Result<(), Self::Error>;
    fn mint(who: &AccountId, asset: &AssetId, amount: Amount) -> Result<(), Self::Error>;
}

/// Metadata provider per asset (names, symbols, etc.).
pub trait AssetMetadataProvider<AssetId> {
    fn name(asset: AssetId) -> Vec<u8>;
    fn symbol(asset: AssetId) -> Vec<u8>;
    fn decimals(asset: AssetId) -> u8;
    fn contract_uri(asset: AssetId) -> Vec<u8>;
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

    /// Mint: prove v ≥ 0, update pending(to) and total supply.
    /// The prover chooses a fresh ElGamal nonce for the minted ciphertext.
    /// Returns (to_new_pending_commit, total_new_commit, minted_ciphertext_64B).
    fn verify_mint(
        asset: &[u8],
        to_pk: &PublicKeyBytes,
        to_old_pending: &[u8],
        total_old: &[u8],
        amount_be: &[u8], // SCALE-encoded T::Balance (you used .using_encoded)
        proof: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>, EncryptedAmount), ()>;

    /// Burn: prove ciphertext encrypts v under `from_pk`, v ≥ 0,
    /// and update available(from) and total supply downward by v.
    /// Returns (from_new_available_commit, total_new_commit, disclosed_amount_u64).
    fn verify_burn(
        asset: &[u8],
        from_pk: &PublicKeyBytes,
        from_old_available: &[u8],
        total_old: &[u8],
        amount_ciphertext: &EncryptedAmount,
        proof: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>, u64), ()>;

    /// Optional disclosure
    fn disclose(asset: &[u8], who_pk: &[u8], cipher: &[u8]) -> Result<u64, Self::Error>;
}

// Operator

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

// ACL

#[derive(Clone, Copy, Encode, Decode, scale_info::TypeInfo)]
pub enum Op {
    Mint,
    Burn,
    Transfer,
    TransferFrom,
    Shield,   // public -> confidential
    Unshield, // confidential -> public
    AcceptPending,
    SetOperator,
}

#[derive(Encode, Decode, scale_info::TypeInfo, Default)]
pub struct AclCtx<Balance, AccountId, AssetId> {
    pub amount: Balance, // plaintext amount if relevant; 0 if not
    pub asset: AssetId,
    pub caller: AccountId,               // origin who signed the extrinsic
    pub owner: Option<AccountId>,        // on-behalf-of (transfer_from etc.)
    pub counterparty: Option<AccountId>, // receiver/sender if applicable
    pub opaque: sp_std::vec::Vec<u8>,    // future-proof (proof bytes, memo, etc.)
}

pub trait AclProvider<AccountId, AssetId, Balance> {
    /// Return Ok(()) to allow; Err(..) to block.
    fn authorize(op: Op, ctx: &AclCtx<Balance, AccountId, AssetId>) -> Result<(), DispatchError>;
}

impl<AccountId, AssetId, Balance> AclProvider<AccountId, AssetId, Balance> for () {
    #[inline]
    fn authorize(_: Op, _: &AclCtx<Balance, AccountId, AssetId>) -> Result<(), DispatchError> {
        Ok(())
    }
}
