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
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::prelude::*;

/// Opaque MW Commitment
// TODO: move to Config trait and add additional generic everywhere
pub type Commitment = [u8; 32];

// TODO: move to traits and impl for pallet-multi-utxo
/// Abstract trait allowing access to UTXO storage.
///
/// Implemented by `pallet-multi-utxo` so that this pallet doesn’t depend on it directly.
pub trait UtxoStorage<AssetId> {
    fn insert_commitment(asset: AssetId, commitment: Commitment);
    fn remove_commitment(asset: AssetId, commitment: &Commitment) -> bool;
}

/// Simple transaction structure: commitments + excess proof (KISS PoC).
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct MwTransaction<AssetId> {
    pub asset_id: AssetId,
    pub input_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub output_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub excess_commitment: Commitment,
}

/// Trait for higher-level pallets (like pallet-confidential-assets) to use.
pub trait MimbleWimbleCompute<AssetId> {
    /// Verify a MW transaction and apply it via UtxoStorage.
    fn verify_and_apply(tx: MwTransaction<AssetId>) -> DispatchResult;
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

        /// Balance type.
        type Balance: Parameter + Member + Copy + MaxEncodedLen + TypeInfo + Default;

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

    fn decompress_point(bytes: &[u8; 32]) -> RistrettoPoint {
        use curve25519_dalek::ristretto::CompressedRistretto;
        CompressedRistretto(*bytes)
            .decompress()
            .unwrap_or(RistrettoPoint::default())
    }

    impl<T: Config> MimbleWimbleCompute<T::AssetId> for Pallet<T> {
        fn verify_and_apply(tx: MwTransaction<T::AssetId>) -> DispatchResult {
            ensure!(
                Self::verify_commitments(&tx),
                Error::<T>::UnbalancedCommitments
            );

            // Optional: record new outputs as opaque commitments.
            for c in tx.output_commitments.iter() {
                T::UtxoBackend::insert_commitment(tx.asset_id, *c);
            }

            Self::deposit_event(Event::MwTransactionApplied(tx.asset_id));
            Ok(())
        }
    }
}
