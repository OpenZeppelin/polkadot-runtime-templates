#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-zk-elgamal (backend-only, ZK/crypto + storage)
//!
//! A minimal **backend pallet** that implements the `ConfidentialBackend` trait from
//! `pallet-confidential-assets`. It stores:
//! - per-account **public keys** (ElGamal-like),
//! - per-(asset,account) **balance ciphertexts/commitments** (opaque),
//! - per-asset **total supply** (opaque).
//!
//! It provides **no public dispatchables**: all mutation flows through the
//! `ConfidentialBackend` trait methods called by `pallet-confidential-assets`.
//!
//! ## Verifier
//! The cryptography is abstracted by a `ZkVerifier` associated type.

use ca_primitives::*;
use frame_support::{pallet_prelude::*, Blake2_128Concat};
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        //type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset type (e.g., u32, U256, etc.).
        type AssetId: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo;

        /// ZK verifier to validate/apply transfers.
        type Verifier: ZkVerifier;
    }

    /// Per-account public key (ElGamal-like).
    #[pallet::storage]
    pub type PublicKey<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, PublicKeyBytes, OptionQuery>;

    /// Opaque balance ciphertext per (asset, account).
    #[pallet::storage]
    pub type BalanceCipher<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AccountId,
        EncryptedAmount, // opaque
        OptionQuery,
    >;

    /// Opaque total supply per asset.
    #[pallet::storage]
    pub type TotalSupplyCipher<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, EncryptedAmount, OptionQuery>;

    // No public extrinsics: backend-only.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        NoPublicKey,
        InvalidProof,
        BackendPolicy,
        BadCipher,
    }

    impl<T: Config> ConfidentialBackend<T::AccountId, T::AssetId> for Pallet<T> {
        // --- Key management ---
        fn set_public_key(
            who: &T::AccountId,
            elgamal_pk: &PublicKeyBytes,
        ) -> Result<(), DispatchError> {
            // Basic sanity: allow non-empty key of bounded size
            ensure!(!elgamal_pk.is_empty(), Error::<T>::BadCipher);
            PublicKey::<T>::insert(who, elgamal_pk.clone());
            Ok(())
        }

        // --- Read helpers ---
        fn total_supply(asset: T::AssetId) -> EncryptedAmount {
            TotalSupplyCipher::<T>::get(asset)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
        }

        fn balance_of(asset: T::AssetId, who: &T::AccountId) -> EncryptedAmount {
            BalanceCipher::<T>::get(asset, who)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap())
        }

        // --- Core ops ---
        fn transfer_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            let from_old = BalanceCipher::<T>::get(asset, from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let to_old = BalanceCipher::<T>::get(asset, to)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let total_old = TotalSupplyCipher::<T>::get(asset)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            // Verify & compute new opaque balances (and new total if needed).
            let (from_new_raw, to_new_raw, total_new_raw) = T::Verifier::verify_transfer_and_apply(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                &from_old,
                &to_old,
                &encrypted_amount,
                &input_proof,
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            // Rebound to our bounded types
            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new: EncryptedAmount =
                BoundedVec::try_from(to_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let total_new: EncryptedAmount =
                BoundedVec::try_from(total_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            ensure!(
                total_new.as_slice() == total_old.as_slice(),
                Error::<T>::InvalidProof
            );

            BalanceCipher::<T>::insert(asset, from, from_new.clone());
            BalanceCipher::<T>::insert(asset, to, to_new.clone());
            TotalSupplyCipher::<T>::insert(asset, total_new.clone());

            // The front pallet emits the event; we just return the encrypted "transferred" amount.
            // For KISS we return the same envelope; a real impl may return a canonicalized ciphertext.
            Ok(BoundedVec::try_from(encrypted_amount.into_inner()).expect("len checked"))
        }

        fn transfer_acl(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            amount: EncryptedAmount,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            let from_old = BalanceCipher::<T>::get(asset, from)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let to_old = BalanceCipher::<T>::get(asset, to)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());
            let _total_old = TotalSupplyCipher::<T>::get(asset)
                .unwrap_or_else(|| BoundedVec::try_from(Vec::new()).ok().unwrap());

            let (from_new_raw, to_new_raw, total_new_raw) = T::Verifier::apply_acl_transfer(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                &from_old,
                &to_old,
                &amount,
            )
            .map_err(|_| Error::<T>::BackendPolicy)?;

            let from_new: EncryptedAmount =
                BoundedVec::try_from(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new: EncryptedAmount =
                BoundedVec::try_from(to_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let total_new: EncryptedAmount =
                BoundedVec::try_from(total_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            BalanceCipher::<T>::insert(asset, from, from_new.clone());
            BalanceCipher::<T>::insert(asset, to, to_new.clone());
            TotalSupplyCipher::<T>::insert(asset, total_new.clone());

            Ok(amount)
        }

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
}
