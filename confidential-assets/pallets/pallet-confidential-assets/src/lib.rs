#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-confidential-assets (MW-backed, KISS)
//!
//! User-facing pallet that **submits confidential transfers** by delegating to a
//! MimbleWimble verification/apply backend via an **abstract trait**.
//!
//! ## Layers
//! - `pallet-multi-utxo` → stores opaque commitments by asset.
//! - `pallet-mimblewimble` → verifies MW txs and writes commitments via UTXO backend.
//! - `pallet-confidential-assets` (this pallet) → user & policy surface (operators, submission).
//!
//! ## What this pallet does now (v0 KISS)
//! - Optional **operator** registry: `(holder, asset, operator) -> until_block`.
//! - A single extrinsic `submit_confidential_tx(tx)` that forwards an `MwTransaction<AssetId>`
//!   to the MW backend (`MimbleWimbleCompute`) for verification & application.
//!
//! ## What it intentionally doesn't do (yet)
//! - No account-indexed balances or plaintext amounts (that’s not MW).
//! - No mint/burn helpers; do those with specialized MW txs if you need them.
//! - No range-proof checks here (they belong in the MW backend).
//!
//! Extend later with: policy hooks, fees, disclosures, operator-mediated submits, etc.

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::prelude::*;

// Bring in the generic MW trait + types (no direct pallet coupling required).
// TODO: move to common primitives
pub type Commitment = [u8; 32];
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

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type used across the stack.
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        /// Backend that verifies & applies confidential txs (MimbleWimble).
        ///
        /// Typically: `type MwBackend = pallet_mimblewimble::Pallet<Self>;`
        type MwBackend: MimbleWimbleCompute<Self::AssetId>;

        /// Weight info
        type WeightInfo: WeightData;
    }

    // Keep a small weight trait KISS
    pub trait WeightData {
        fn set_operator() -> Weight;
        fn submit_confidential_tx(inputs: u32, outputs: u32) -> Weight;
    }
    impl WeightData for () {
        fn set_operator() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn submit_confidential_tx(_inputs: u32, _outputs: u32) -> Weight {
            // Replace with real benches; scale by inputs/outputs if you like.
            Weight::from_parts(20_000, 0)
        }
    }

    /// Storage for operators:
    /// (Holder, AssetId, Operator) => Until
    #[pallet::storage]
    pub type Operators<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>, // Holder
            NMapKey<Blake2_128Concat, T::AssetId>,   // AssetId
            NMapKey<Blake2_128Concat, T::AccountId>, // Operator
        ),
        BlockNumberFor<T>, // valid until this block
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        OperatorSet {
            asset: T::AssetId,
            holder: T::AccountId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        },
        /// A confidential transaction (MW tx) was submitted and accepted.
        ConfidentialTxApplied { asset: T::AssetId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Operator is not authorized (holder, asset, operator, block height mismatch).
        NotAuthorized,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set/extend an operator for a holder & asset.
        ///
        /// Per ERC-7984 patterns, we do **not** check `until` against current block for writes;
        /// readers should enforce validity at use-time to save weight here.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_operator())]
        pub fn set_operator(
            origin: OriginFor<T>,
            asset: T::AssetId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            Operators::<T>::insert((holder.clone(), asset, operator.clone()), until);
            Self::deposit_event(Event::OperatorSet {
                asset,
                holder,
                operator,
                until,
            });
            Ok(())
        }

        /// Submit a **MimbleWimble** confidential transaction for verification & application.
        ///
        /// - Anyone may submit; the MW signatures/proofs inside the tx prove legitimacy.
        /// - If you want **operator-submitted** flows, enforce it off-chain or add a second
        ///   call that checks `Operators` before forwarding to the backend.
        #[pallet::call_index(1)]
        #[pallet::weight({
            // If you want quick linear scaling by tx size:
            let ins = tx.input_commitments.len() as u32;
            let outs = tx.output_commitments.len() as u32;
            T::WeightInfo::submit_confidential_tx(ins, outs)
        })]
        pub fn submit_confidential_tx(
            origin: OriginFor<T>,
            tx: MwTransaction<T::AssetId>,
        ) -> DispatchResult {
            // Signed only for spam control; the account is not semantically used by MW.
            let _ = ensure_signed(origin)?;

            // Delegate to the generic MimbleWimble backend.
            <T as Config>::MwBackend::verify_and_apply(tx.clone())?;

            // Emit a minimal event keyed by asset (pull anything else you want from `tx`).
            Self::deposit_event(Event::ConfidentialTxApplied { asset: tx.asset_id });
            Ok(())
        }
    }

    // --- Optional helper: operator gate (use this for an operator-only submit if you want) ---

    impl<T: Config> Pallet<T> {
        /// Example helper to check whether `operator` is authorized to act for `holder` on `asset`
        /// at current block number. You can call this in a separate operator-only extrinsic
        /// before forwarding the tx to the MW backend.
        #[allow(dead_code)]
        fn is_operator_allowed(
            holder: &T::AccountId,
            asset: &T::AssetId,
            operator: &T::AccountId,
            now: BlockNumberFor<T>,
        ) -> bool {
            match Operators::<T>::get((holder, asset, operator)) {
                Some(until) => now <= until,
                None => false,
            }
        }
    }
}
