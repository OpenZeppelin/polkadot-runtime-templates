#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-mimblewimble (KISS)
//!
//! Provides **confidential multi-asset transfers** on top of [`pallet-multi-utxo`].
//! Verifies that MW transactions balance (no inflation) and performs **cut-through**
//! before committing to the underlying UTXO storage via an abstract `UtxoStorage` trait.
//!
//! ## Key Traits
//! - [`UtxoStorage`] — abstract storage access to UTXO sets (implemented by pallet-multi-utxo).
//! - [`MimbleWimbleCompute`] — main interface used by higher pallets (e.g. confidential-assets).
//!
//! ## Responsibilities
//! - Verify Pedersen commitment sums (input/output balance).
//! - Perform cut-through (remove spent outputs from the transaction).
//! - Record new UTXOs via `UtxoStorage`.
//!
//! ## Non-goals
//! - Range proof verification (future work).
//! - Signature aggregation (future work).
//!
//! ## Features
//! - `no_std` ready (uses curve25519-dalek `u64_backend`).
//! - Independent of any specific storage layout.
//!
//! KISS: correctness & isolation > optimization for now.

use curve25519_dalek::ristretto::RistrettoPoint;
use frame_support::{ensure, pallet_prelude::*, BoundedVec};
use frame_system::pallet_prelude::*;
use parity_scale_codec::MaxEncodedLen;
use primitives_confidential_assets::{
    Commitment, ConfidentialBackend, EncryptedAmount, ExternalEncryptedAmount, InputProof,
    MimbleWimbleCompute, MwTransaction, UtxoStorage,
};
use scale_info::TypeInfo;
use sp_std::prelude::*;

// Helper: compressed Ristretto sum (same decompress helper you already have)
pub(crate) fn sum_points(cs: impl Iterator<Item = Commitment>) -> Commitment {
    use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
    let sum = cs
        .map(|c| {
            CompressedRistretto(c)
                .decompress()
                .unwrap_or(RistrettoPoint::default())
        })
        .fold(RistrettoPoint::default(), |a, b| a + b);
    sum.compress().to_bytes()
}

/// Convert a `[u8; 32]` commitment into `EncryptedAmount` (BoundedVec<u8, 128>).
#[inline]
pub(crate) fn c32_to_cipher(c: Commitment) -> EncryptedAmount {
    // 32 <= 128 so this infallible expect is fine in KISS PoC.
    BoundedVec::try_from(c.to_vec()).expect("32 <= MaxCiphertextLen; qed")
}

/// Interpret an `EncryptedAmount` as a compressed Ristretto point (must be 32 bytes).
#[inline]
pub(crate) fn cipher_to_c32(a: &EncryptedAmount) -> Result<Commitment, DispatchError> {
    if a.len() != 32 {
        return Err(DispatchError::Other("EncryptedAmountMustBe32Bytes"));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&a[..]);
    Ok(out)
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type.
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        /// Hook into external UTXO storage.
        type UtxoBackend: UtxoStorage<Self::AssetId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A valid MimbleWimble tx was applied.
        MwTransactionApplied(T::AssetId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// MW commitments do not balance (invalid excess).
        UnbalancedCommitments,
        /// Generic verification failure.
        InvalidTransaction,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a MimbleWimble transaction for verification & cut-through.
        #[pallet::call_index(0)]
        #[pallet::weight(<Pallet<T>>::weight_for_submit())]
        pub fn submit(origin: OriginFor<T>, tx: MwTransaction<T::AssetId>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            Self::verify_and_apply(tx)
        }
    }

    impl<T: Config> Pallet<T> {
        /// Dummy linear weight placeholder (replaces deprecated constant).
        fn weight_for_submit() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        /// Check MW balance condition: sum(outputs) - sum(inputs) == excess
        fn verify_commitments(tx: &MwTransaction<T::AssetId>) -> bool {
            fn decompress_point(bytes: &[u8; 32]) -> RistrettoPoint {
                use curve25519_dalek::ristretto::CompressedRistretto;
                CompressedRistretto(*bytes)
                    .decompress()
                    .unwrap_or(RistrettoPoint::default())
            }
            let sum_inputs = tx
                .input_commitments
                .iter()
                .map(|c| decompress_point(c))
                .fold(RistrettoPoint::default(), |a, b| a + b);

            let sum_outputs = tx
                .output_commitments
                .iter()
                .map(|c| decompress_point(c))
                .fold(RistrettoPoint::default(), |a, b| a + b);

            let diff = sum_outputs - sum_inputs;
            let excess = decompress_point(&tx.excess_commitment);
            diff == excess
        }
    }

    impl<T: Config> MimbleWimbleCompute<T::AssetId> for Pallet<T> {
        fn verify_and_apply(tx: MwTransaction<T::AssetId>) -> DispatchResult {
            ensure!(
                Self::verify_commitments(&tx),
                Error::<T>::UnbalancedCommitments
            );

            // Cut-through: remove spent inputs; ignore if absent.
            for c in tx.input_commitments.iter() {
                let _ = T::UtxoBackend::remove_commitment(tx.asset_id, c);
            }
            // Record new outputs.
            for c in tx.output_commitments.iter() {
                T::UtxoBackend::insert_commitment(tx.asset_id, *c);
            }

            Self::deposit_event(Event::MwTransactionApplied(tx.asset_id));
            Ok(())
        }
    }

    // Consider extracting into abstract ProofInterpreter trait assigned at runtime
    impl<T: Config> Pallet<T> {
        /// Parse `encrypted_amount` + `input_proof` into a concrete MW tx (KISS PoC).
        /// - Uses ONE input (first owned UTXO).
        /// - Outputs: [ amount_to_to , exact_change_back_to_from (= input itself) ].
        /// - Excess = amount_to_to.
        fn to_mw_tx(
            asset: T::AssetId,
            from: &T::AccountId,
            _to: &T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            input_proof: InputProof,
        ) -> Result<MwTransaction<T::AssetId>, DispatchError> {
            // Silence `unused` in this KISS PoC (you'll enforce these later).
            let _ = input_proof;

            // 1) Pick ONE input owned by `from`.
            let input_commit = T::UtxoBackend::iter_owned(asset, from)
                .next()
                .ok_or(DispatchError::Other("InsufficientFundsOrACL"))?;

            // 2) Convert the requested transfer amount into a 32-byte commitment.
            //    (We require `encrypted_amount.len() == 32` in this PoC.)
            let out_commit = super::cipher_to_c32(&encrypted_amount)?;

            // 3) Build MW tx:
            //    inputs  = [input_commit]
            //    outputs = [out_commit, input_commit]  // "change" is re-inserting the input
            //    excess  = out_commit
            use frame_support::BoundedVec;

            let inputs = BoundedVec::try_from(vec![input_commit])
                .map_err(|_| DispatchError::Other("TooManyInputs"))?;

            let outputs = BoundedVec::try_from(vec![out_commit, input_commit])
                .map_err(|_| DispatchError::Other("TooManyOutputs"))?;

            let tx = MwTransaction::<T::AssetId> {
                asset_id: asset,
                input_commitments: inputs,
                output_commitments: outputs,
                excess_commitment: out_commit,
            };

            Ok(tx)
        }

        /// Optional: try to reveal a plaintext amount if policy allows.
        fn try_disclose(
            _asset: T::AssetId,
            cipher: &EncryptedAmount,
            _who: &T::AccountId,
        ) -> Result<u64, DispatchError> {
            // KISS PoC rule:
            // - If `cipher` is exactly 8 bytes, interpret as LE u64 (plaintext path).
            // - Else (e.g., 32-byte MW commitments), not decryptable here.
            match cipher.len() {
                8 => {
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&cipher[..8]);
                    Ok(u64::from_le_bytes(buf))
                }
                _ => Err(DispatchError::Other("NotDecryptableInPoC")),
            }
        }
    }

    impl<T: Config> ConfidentialBackend<T::AccountId, T::AssetId> for Pallet<T> {
        fn total_supply(asset: T::AssetId) -> EncryptedAmount {
            // Sum all unspent commitments for this asset and wrap as EncryptedAmount.
            let c = sum_points(T::UtxoBackend::iter_unspent(asset));
            super::c32_to_cipher(c)
        }

        fn balance_of(asset: T::AssetId, who: &T::AccountId) -> EncryptedAmount {
            // Sum unspent commitments owned by `who` for this asset.
            let c = sum_points(T::UtxoBackend::iter_owned(asset, who));
            super::c32_to_cipher(c)
        }

        fn transfer_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            // 1) Build a concrete MW tx from the opaque envelope+proof.
            let tx = Self::to_mw_tx(asset, from, to, encrypted_amount, input_proof)?;

            // 2) Verify & apply via your existing pallet API.
            Self::verify_and_apply(tx.clone())?;

            // 3) Return the “amount” as ciphertext: wrap [u8;32] -> BoundedVec<u8, 128>.
            Ok(super::c32_to_cipher(tx.excess_commitment))
        }

        fn transfer_acl(
            asset: T::AssetId,
            from: &T::AccountId,
            _to: &T::AccountId,
            amount: EncryptedAmount,
        ) -> Result<EncryptedAmount, DispatchError> {
            // KISS PoC: spend one owned input and produce ONE output equal to `amount`.
            // We treat `amount` bytes as a compressed commitment ([u8; 32]).
            use frame_support::BoundedVec;

            // Pick a single input owned by `from`.
            let mut inputs: Vec<Commitment> = Vec::new();
            if let Some(first) = T::UtxoBackend::iter_owned(asset, from).next() {
                inputs.push(first);
            }
            ensure!(
                !inputs.is_empty(),
                DispatchError::Other("InsufficientFundsOrACL")
            );

            // Convert `amount` -> commitment (must be exactly 32 bytes).
            let out_commit = super::cipher_to_c32(&amount)?;

            // Build minimal tx: 1 input -> 1 output
            let tx = MwTransaction::<T::AssetId> {
                asset_id: asset,
                input_commitments: BoundedVec::try_from(inputs)
                    .map_err(|_| DispatchError::Other("TooManyInputs"))?,
                output_commitments: BoundedVec::try_from(vec![out_commit])
                    .map_err(|_| DispatchError::Other("TooManyOutputs"))?,
                // For this PoC, treat `amount` commitment as the "excess".
                excess_commitment: out_commit,
            };

            // Verify & apply (performs cut-through + inserts output).
            <Self as MimbleWimbleCompute<T::AssetId>>::verify_and_apply(tx)?;

            // Return the same encrypted amount we were asked to transfer.
            Ok(amount)
        }

        fn disclose_amount(
            asset: T::AssetId,
            encrypted_amount: &EncryptedAmount,
            who: &T::AccountId,
        ) -> Result<u64, DispatchError> {
            Self::try_disclose(asset, encrypted_amount, who)
        }
    }
}
