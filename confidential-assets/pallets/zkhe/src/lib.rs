#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-zkhe (backend-only, ZK/crypto + storage)
//!
//! Backend pallet that implements the `ConfidentialBackend` trait from
//! `pallet-confidential-assets`. It stores:
//! - per-account **public keys**,
//! - per-(asset,account) **available balance** ciphertext,
//! - per-(asset,account) **pending balance** ciphertext,
//! - per-asset **total supply** ciphertext.
//!
//! Dispatchables:
//! - `accept_pending`: receiver-side acceptance that verifies a provided delta+proof,
//!   zeroes pending, and updates available to the verified sum.
//!
//! Notes:
//! - Proof verification is delegated to the `ZkVerifier` associated type.
//! - Sender-side transfers only update: sender.available ↓ and receiver.pending ↑ (by
//!   setting receiver.pending to the new post-delta ciphertext, accumulating across
//!   transfers by using pending-as-base when present).
//!
//! This pallet remains a backend; front pallets should emit user-facing events.

use confidential_assets_primitives::*;
use frame_support::{pallet_prelude::*, Blake2_128Concat};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type.
        type AssetId: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo;

        /// ZK verifier to validate/apply transfers and receiver acceptance.
        type Verifier: ZkVerifier;

        /// Call weights
        type WeightInfo: WeightData;
    }

    /// Weight
    pub trait WeightData {
        fn transfer() -> Weight;
        fn transfer_from_available() -> Weight;
        fn accept_pending() -> Weight;
    }
    impl WeightData for () {
        fn transfer() -> Weight {
            Weight::from_parts(20_000, 0)
        }
        fn transfer_from_available() -> Weight {
            Weight::from_parts(22_000, 0)
        }
        fn accept_pending() -> Weight {
            Weight::from_parts(25_000, 0)
        }
    }

    // -------------------- Storage --------------------

    /// Per-account public key (ElGamal-like).
    #[pallet::storage]
    pub type PublicKey<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, PublicKeyBytes, OptionQuery>;

    /// **Available** balance ciphertext per (asset, account).
    #[pallet::storage]
    pub type AvailableBalanceCipher<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AccountId,
        EncryptedAmount, // opaque bytes
        OptionQuery,
    >;

    /// **Pending** balance ciphertext per (asset, account).
    ///
    /// Semantics:
    /// - If `Some(x)`, it is the receiver's **post-accumulation** ciphertext after applying
    ///   all yet-unaccepted deltas to the prior available.
    /// - When additional transfers arrive, sender-side verification uses `pending` as
    ///   the receiver "old" value (if present) to keep accumulating.
    #[pallet::storage]
    pub type PendingBalanceCipher<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AccountId,
        EncryptedAmount, // opaque bytes
        OptionQuery,
    >;

    /// Opaque total supply per asset.
    #[pallet::storage]
    pub type TotalSupplyCipher<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, EncryptedAmount, OptionQuery>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // -------------------- Events / Errors --------------------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pending accepted: (who, asset)
        PendingAccepted(T::AccountId, T::AssetId),
        /// Public key set: (who)
        PublicKeySet(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoPublicKey,
        InvalidProof,
        BackendPolicy,
        BadCipher,
        NoPending,
        SupplyMismatch,
    }

    // -------------------- Dispatchables --------------------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::transfer())]
        pub fn transfer(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let _transferred_amt =
                Self::transfer_encrypted(asset, &from, &to, encrypted_amount, proof)?;
            // todo emit event
            Ok(())
        }
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::transfer_from_available())]
        pub fn transfer_from_available(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let _transferred_amt =
                Self::do_transfer_from_available(asset, &from, &to, encrypted_amount, proof)?;
            // todo emit event
            Ok(())
        }
        /// Accept all pending for `(asset, origin)` by verifying a receiver-side proof
        /// over the provided delta-ciphertext and moving:
        /// - `available := verified_to_new`
        /// - `pending := None`
        ///
        /// Inputs:
        /// - `asset`: target asset id
        /// - `encrypted_amount`: the Δ ElGamal ciphertext (opaque envelope from sender phase)
        /// - `proof`: receiver-side acceptance proof/envelope
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::accept_pending())]
        pub fn accept_pending(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            encrypted_amount: ExternalEncryptedAmount, //may be unnecessary if in proof envelope already
            proof: InputProof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_accept_pending(who, asset, encrypted_amount, proof)
        }
    }

    // -------------------- Backend Trait Impl --------------------

    impl<T: Config> ConfidentialBackend<T::AccountId, T::AssetId> for Pallet<T> {
        // --- Key management ---
        fn set_public_key(
            who: &T::AccountId,
            elgamal_pk: &PublicKeyBytes,
        ) -> Result<(), DispatchError> {
            ensure!(!elgamal_pk.is_empty(), Error::<T>::BadCipher);
            PublicKey::<T>::insert(who, elgamal_pk.clone());
            Self::deposit_event(Event::PublicKeySet(who.clone()));
            Ok(())
        }

        // --- Read helpers ---
        fn total_supply(asset: T::AssetId) -> EncryptedAmount {
            TotalSupplyCipher::<T>::get(asset)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
        }

        fn balance_of(asset: T::AssetId, who: &T::AccountId) -> EncryptedAmount {
            // For compatibility, expose the **available** balance via this helper.
            AvailableBalanceCipher::<T>::get(asset, who)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
        }

        // --- Core ops ---

        /// Sender-initiated confidential transfer:
        /// - TODO: if pending(from).is_some then from.accept_pending() first.
        /// - Sender available decreases according to proof-verified delta.
        /// - Receiver **pending** is set to the new post-delta ciphertext, using the
        ///   current pending (if any) as the "old" base to accumulate multiple in-flight transfers.
        fn transfer_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            fn split_proof(_p: InputProof) -> (InputProof, InputProof) {
                todo!()
            }
            let (accept_proof, send_proof) = split_proof(input_proof);
            if let Some(delta) = PendingBalanceCipher::<T>::get(asset, &from) {
                Self::do_accept_pending(from.clone(), asset, delta, accept_proof)?;
            }
            Self::do_transfer_from_available(asset, from, to, encrypted_amount, send_proof)
        }

        /// Policy/ACL transfer path **without ZK proof** (old signature).
        ///
        /// Behavior mirrors `transfer_encrypted` semantics:
        /// - Sender available updated from `from_old_avail`.
        /// - Receiver pending uses `pending` as base when present, else `available`.
        fn transfer_acl(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            amount: EncryptedAmount,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            let from_old_avail = AvailableBalanceCipher::<T>::get(asset, from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            let to_base = PendingBalanceCipher::<T>::get(asset, to).unwrap_or_else(|| {
                AvailableBalanceCipher::<T>::get(asset, to)
                    .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
            });

            let (from_new_raw, to_new_raw) = T::Verifier::acl_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail.as_slice(),
                to_base.as_slice(),
                amount.as_slice(),
            )
            .map_err(|_| Error::<T>::BackendPolicy)?;

            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_accumulated: EncryptedAmount =
                BoundedVec::try_from(to_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            // Commit:
            AvailableBalanceCipher::<T>::insert(asset, from, from_new);
            PendingBalanceCipher::<T>::insert(asset, to, to_new_accumulated);

            Ok(amount)
        }

        /// Optional disclosure (policy dependent). Returns plaintext amount or error.
        fn disclose_amount(
            asset: T::AssetId,
            encrypted_amount: &EncryptedAmount,
            who: &T::AccountId,
        ) -> Result<u64, DispatchError> {
            let pk = PublicKey::<T>::get(who).ok_or(Error::<T>::NoPublicKey)?;
            T::Verifier::disclose(
                &asset.using_encoded(|b| b.to_vec()),
                &pk,
                encrypted_amount.as_slice(),
            )
            .map_err(|_| Error::<T>::BackendPolicy.into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Sender-initiated confidential transfer:
        /// - Sender available decreases according to proof-verified delta.
        /// - Receiver **pending** is set to the new post-delta ciphertext, using the
        ///   current pending (if any) as the "old" base to accumulate multiple in-flight transfers.
        fn do_transfer_from_available(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            // Old balances:
            let from_old_avail = AvailableBalanceCipher::<T>::get(asset, from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            // Receiver base = pending if present, else available.
            let to_base = PendingBalanceCipher::<T>::get(asset, to).unwrap_or_else(|| {
                AvailableBalanceCipher::<T>::get(asset, to)
                    .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
            });

            // Verify & compute new opaque balances (total unchanged on transfer).
            let (from_new_raw, to_new_raw) = T::Verifier::verify_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail.as_slice(),
                to_base.as_slice(),
                encrypted_amount.as_slice(),
                input_proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_accumulated: EncryptedAmount =
                BoundedVec::try_from(to_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            // Commit:
            // - Sender available := from_new
            // - Receiver pending := to_new_accumulated (accumulates over time)
            AvailableBalanceCipher::<T>::insert(asset, from, from_new);
            PendingBalanceCipher::<T>::insert(asset, to, to_new_accumulated);

            // Return canonicalized delta envelope back to caller/front pallet.
            Ok(BoundedVec::try_from(encrypted_amount.into_inner()).expect("len checked"))
        }

        fn do_accept_pending(
            who: T::AccountId,
            asset: T::AssetId,
            _delta_ct: ExternalEncryptedAmount, //included in proof envelope?
            proof: InputProof,
        ) -> DispatchResult {
            let to_pk = PublicKey::<T>::get(&who).ok_or(Error::<T>::NoPublicKey)?;

            // Current available and pending states
            let to_avail = AvailableBalanceCipher::<T>::get(asset, &who)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            let _to_pending =
                PendingBalanceCipher::<T>::get(asset, &who).ok_or(Error::<T>::NoPending)?;
            // TODO: need to check that if the proof contains delta_ct then
            // delta_ct == to_pending
            // Verify receiver acceptance: computes (to_new, total_new)
            let to_new_raw = T::Verifier::verify_transfer_received(
                &asset.using_encoded(|b| b.to_vec()),
                &to_pk,
                to_avail.as_slice(),
                proof.as_slice(), // does verification need delta_ct or is it included in proof envelope
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            // Bound them
            let to_new: EncryptedAmount =
                BoundedVec::try_from(to_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            // Commit state: available := verified new; pending := None.
            AvailableBalanceCipher::<T>::insert(asset, &who, to_new);
            PendingBalanceCipher::<T>::remove(asset, &who);

            Self::deposit_event(Event::PendingAccepted(who, asset));
            Ok(())
        }
    }
}
