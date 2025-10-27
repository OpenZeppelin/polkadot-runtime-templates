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

    /// **Pending** balance deposits per (asset, account).
    #[pallet::storage]
    pub type PendingDeposits<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>, // holder
            NMapKey<Blake2_128Concat, T::AssetId>,   // asset
            NMapKey<Blake2_128Concat, u64>,          // PendingDepositId
        ),
        EncryptedAmount,
        OptionQuery,
    >;

    #[pallet::storage]
    pub type NextPendingDepositId<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AssetId,
        u64, //pub type PendingDepositId = u64
        ValueQuery,
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
        /// Accept pending deposits for `(asset, origin)`
        ///
        /// Inputs:
        /// - `asset`: target asset id
        /// - `deposits`: deposits that are being accepted from pending deposits
        /// - `encrypted_amount`: the Δ ElGamal ciphertext (opaque envelope from sender phase)
        /// - `proof`: receiver-side acceptance proof/envelope
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::accept_pending())]
        pub fn accept_pending(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            deposits: Vec<u64>, //deposit ids
            encrypted_amount: ExternalEncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let to = ensure_signed(origin)?;
            Self::do_accept_pending(to, asset, deposits, encrypted_amount, proof)
        }
        /// Extrinsic for advanced users to make pending deposits immediately
        /// spendable for transfer (vs. split call for accept_pending + transfer)
        /// Requires specifying the pending deposits that the sender intends to make liquid
        /// before transferring a separate amount out of their available balance.
        // TODO: make extrinsic atomic so either fails or doesn't write to storage
        // TODO: impl could be more efficient if logic shared between paths
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::transfer_from_available())] //TODO
        pub fn accept_pending_and_transfer(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            to: T::AccountId,
            deposit_ids: Vec<u64>, //deposit ids
            accept_sum: ExternalEncryptedAmount,
            accept_proof: InputProof,
            transfer_amount: ExternalEncryptedAmount,
            transfer_proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            Self::do_accept_pending(from.clone(), asset, deposit_ids, accept_sum, accept_proof)?;
            let _transferred_amt =
                Self::transfer_encrypted(asset, &from, &to, transfer_amount, transfer_proof)?;
            // todo emit event
            Ok(())
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
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            // Old balances:
            let from_old_avail = AvailableBalanceCipher::<T>::get(asset, from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let to_old_pending = PendingBalanceCipher::<T>::get(asset, &to)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            // Verify & compute new opaque balances (total unchanged on transfer).
            let (from_new_raw, to_new_pending_raw) = T::Verifier::verify_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail.as_slice(),
                to_old_pending.as_slice(),
                encrypted_amount.as_slice(),
                input_proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_pending: EncryptedAmount =
                BoundedVec::try_from(to_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;
            // Commit state: update available and pending
            AvailableBalanceCipher::<T>::insert(asset, &from, from_new);
            PendingBalanceCipher::<T>::insert(asset, &from, to_new_pending);
            let deposit_id = NextPendingDepositId::<T>::get(to, &asset);
            PendingDeposits::<T>::insert((to, asset, deposit_id), &encrypted_amount);
            NextPendingDepositId::<T>::insert(to, asset, deposit_id + 1);

            // Return canonicalized delta envelope back to caller/front pallet.
            Ok(BoundedVec::try_from(encrypted_amount.into_inner()).expect("len checked"))
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
            let to_old_pending = PendingBalanceCipher::<T>::get(asset, &to)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            let (from_new_raw, to_new_pending_raw) = T::Verifier::acl_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail.as_slice(),
                to_old_pending.as_slice(),
                amount.as_slice(),
            )
            .map_err(|_| Error::<T>::BackendPolicy)?;

            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_pending: EncryptedAmount =
                BoundedVec::try_from(to_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;
            // Commit state: update available and pending
            AvailableBalanceCipher::<T>::insert(asset, &from, from_new);
            PendingBalanceCipher::<T>::insert(asset, &from, to_new_pending);
            let deposit_id = NextPendingDepositId::<T>::get(to, &asset);
            PendingDeposits::<T>::insert((to, asset, deposit_id), &amount);
            NextPendingDepositId::<T>::insert(to, asset, deposit_id + 1);

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
        fn do_accept_pending(
            from: T::AccountId,
            asset: T::AssetId,
            deposits: Vec<u64>, //DepositId
            deposits_sum: ExternalEncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let from_pk = PublicKey::<T>::get(&from).ok_or(Error::<T>::NoPublicKey)?;

            // Current available and pending states
            let from_avail = AvailableBalanceCipher::<T>::get(asset, &from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let from_pending = PendingBalanceCipher::<T>::get(asset, &from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            let mut deposit_amounts: Vec<u8> = Vec::new();
            for id in deposits.clone() {
                if let Some(deposit) = PendingDeposits::<T>::get((from.clone(), asset, id)) {
                    // TODO: verifier must know how to decode accordingly
                    deposit_amounts.extend_from_slice(deposit.as_slice());
                }
            }
            ensure!(!deposit_amounts.is_empty(), Error::<T>::NoPending);
            let (from_new_raw, from_new_pending_raw) = T::Verifier::verify_transfer_received(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                from_avail.as_slice(),
                from_pending.as_slice(),
                deposit_amounts.as_slice(),
                deposits_sum.as_slice(),
                proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            // Bound them
            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let from_new_pending: EncryptedAmount =
                BoundedVec::try_from(from_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;
            deposits
                .into_iter()
                .for_each(|id| PendingDeposits::<T>::remove((from.clone(), asset, id)));
            // Commit state: update available and pending
            AvailableBalanceCipher::<T>::insert(asset, &from, from_new);
            if from_new_pending.is_empty()
            /* == 0*/
            {
                PendingBalanceCipher::<T>::remove(asset, &from);
            } else {
                PendingBalanceCipher::<T>::insert(asset, &from, from_new_pending);
            }
            Self::deposit_event(Event::PendingAccepted(from, asset));
            Ok(())
        }
    }
}
