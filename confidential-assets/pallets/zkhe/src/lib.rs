#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-zkhe (backend-only, ZK/crypto + storage)
//!
//! Stores:
//! - per-account public key
//! - per-(asset,account) available commitment (32B)
//! - per-(asset,account) pending commitment (32B)
//! - per-asset total supply commitment (32B)
//! - per-(account,asset,id) pending deposits as 64B ElGamal ciphertexts (UTXO-like)
//!
//! Dispatchables:
//! - `accept_pending`: consume selected UTXOs, prove ΔC, move pending → available
//!
//! Notes:
//! - All cryptographic checks live in `Config::Verifier`.
//! - Sender transfer updates: available(from) ↓, pending(to) ↑.

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
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AssetId: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo;

        /// Verifier boundary (no_std on-chain).
        /// - verify_transfer_sent(..) -> (from_new_commit, to_new_pending_commit)
        /// - verify_transfer_received(.., pending_commits: &[[u8;32]], accept_envelope: &[u8])
        type Verifier: ZkVerifier;

        type WeightInfo: WeightData;
    }

    /// Weights
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

    #[pallet::storage]
    pub type PublicKey<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, PublicKeyBytes, OptionQuery>;

    #[pallet::storage]
    pub type AvailableBalanceCommit<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AccountId,
        Commitment,
        OptionQuery,
    >;

    #[pallet::storage]
    pub type PendingBalanceCommit<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AccountId,
        Commitment,
        OptionQuery,
    >;

    /// UTXO-like pending deposits as 64B ElGamal ciphertexts.
    #[pallet::storage]
    pub type PendingDeposits<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, T::AssetId>,
            NMapKey<Blake2_128Concat, u64>,
        ),
        EncryptedAmount, // [u8; 64]
        OptionQuery,
    >;

    #[pallet::storage]
    pub type NextPendingDepositId<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AssetId,
        u64,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type TotalSupplyCommit<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, Commitment, OptionQuery>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // -------------------- Events / Errors --------------------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PendingAccepted(T::AccountId, T::AssetId),
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
            encrypted_amount: EncryptedAmount, // 64B Δciphertext
            proof: InputProof,                 // sender bundle
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let _ = Self::transfer_encrypted(asset, &from, &to, encrypted_amount, proof)?;
            Ok(())
        }

        /// Accept selected UTXO deposits; prove ΔC; update (avail, pending) for caller.
        ///
        /// `accept_envelope` layout (Option A):
        ///   delta_comm(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::accept_pending())]
        pub fn accept_pending(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            deposits: Vec<u64>, // UTXO ids
            accept_envelope: InputProof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_accept_pending(who, asset, deposits, accept_envelope)
        }

        /// Atomic: accept pending then transfer from available.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::transfer_from_available())]
        pub fn accept_pending_and_transfer(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            to: T::AccountId,
            deposit_ids: Vec<u64>,
            accept_envelope: InputProof,
            transfer_amount: EncryptedAmount,
            transfer_proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            Self::do_accept_pending(from.clone(), asset, deposit_ids, accept_envelope)?;
            let _ = Self::transfer_encrypted(asset, &from, &to, transfer_amount, transfer_proof)?;
            Ok(())
        }
    }

    // -------------------- Backend Trait Impl --------------------

    impl<T: Config> ConfidentialBackend<T::AccountId, T::AssetId> for Pallet<T> {
        fn set_public_key(
            who: &T::AccountId,
            elgamal_pk: &PublicKeyBytes,
        ) -> Result<(), DispatchError> {
            ensure!(!elgamal_pk.is_empty(), Error::<T>::BadCipher);
            PublicKey::<T>::insert(who, elgamal_pk.clone());
            Self::deposit_event(Event::PublicKeySet(who.clone()));
            Ok(())
        }

        fn total_supply(asset: T::AssetId) -> [u8; 32] {
            TotalSupplyCommit::<T>::get(asset).unwrap_or([0u8; 32])
        }

        fn balance_of(asset: T::AssetId, who: &T::AccountId) -> [u8; 32] {
            AvailableBalanceCommit::<T>::get(asset, who).unwrap_or([0u8; 32])
        }

        fn transfer_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            // lifetime-safe buffers
            let from_old_avail_opt = AvailableBalanceCommit::<T>::get(asset, from);
            let from_old_avail_buf;
            let from_old_avail: &[u8] = match from_old_avail_opt {
                Some(c) => {
                    from_old_avail_buf = c;
                    &from_old_avail_buf[..]
                }
                None => &[],
            };

            let to_old_pending_opt = PendingBalanceCommit::<T>::get(asset, to);
            let to_old_pending_buf;
            let to_old_pending: &[u8] = match to_old_pending_opt {
                Some(c) => {
                    to_old_pending_buf = c;
                    &to_old_pending_buf[..]
                }
                None => &[],
            };

            let (from_new_raw, to_new_pending_raw) = T::Verifier::verify_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail,
                to_old_pending,
                &encrypted_amount, // Δciphertext bytes
                input_proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let from_new = vec32(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_pending = vec32(to_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;

            AvailableBalanceCommit::<T>::insert(asset, from, from_new);
            PendingBalanceCommit::<T>::insert(asset, to, to_new_pending);

            // record UTXO for receiver
            let id = NextPendingDepositId::<T>::get(to, &asset);
            PendingDeposits::<T>::insert((to, asset, id), encrypted_amount);
            NextPendingDepositId::<T>::insert(to, asset, id + 1);

            Ok(encrypted_amount)
        }

        fn transfer_acl(
            asset: T::AssetId,
            from: &T::AccountId,
            to: &T::AccountId,
            amount: EncryptedAmount,
        ) -> Result<EncryptedAmount, DispatchError> {
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            // lifetime-safe buffers
            let from_old_avail_opt = AvailableBalanceCommit::<T>::get(asset, from);
            let from_old_avail_buf;
            let from_old_avail: &[u8] = match from_old_avail_opt {
                Some(c) => {
                    from_old_avail_buf = c;
                    &from_old_avail_buf[..]
                }
                None => &[],
            };

            let to_old_pending_opt = PendingBalanceCommit::<T>::get(asset, to);
            let to_old_pending_buf;
            let to_old_pending: &[u8] = match to_old_pending_opt {
                Some(c) => {
                    to_old_pending_buf = c;
                    &to_old_pending_buf[..]
                }
                None => &[],
            };

            let (from_new_raw, to_new_pending_raw) = T::Verifier::acl_transfer_sent(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                &to_pk,
                from_old_avail,
                to_old_pending,
                &amount,
            )
            .map_err(|_| Error::<T>::BackendPolicy)?;

            let from_new = vec32(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let to_new_pending = vec32(to_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;

            AvailableBalanceCommit::<T>::insert(asset, from, from_new);
            PendingBalanceCommit::<T>::insert(asset, to, to_new_pending);

            let id = NextPendingDepositId::<T>::get(to, &asset);
            PendingDeposits::<T>::insert((to, asset, id), amount);
            NextPendingDepositId::<T>::insert(to, asset, id + 1);

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
                &encrypted_amount[..],
            )
            .map_err(|_| Error::<T>::BackendPolicy.into())
        }
    }

    // -------------------- Internal helpers --------------------

    impl<T: Config> Pallet<T> {
        /// Build the list of 32B commitments (C) from selected UTXO deposits.
        fn build_pending_commit_list(
            who: &T::AccountId,
            asset: &T::AssetId,
            deposit_ids: &[u64],
        ) -> Result<Vec<[u8; 32]>, Error<T>> {
            ensure!(!deposit_ids.is_empty(), Error::<T>::NoPending);
            let mut out = Vec::with_capacity(deposit_ids.len());
            for &id in deposit_ids {
                let dep = PendingDeposits::<T>::get((who.clone(), *asset, id))
                    .ok_or(Error::<T>::NoPending)?;
                let mut c = [0u8; 32];
                c.copy_from_slice(&dep[0..32]); // C part of ElGamal
                out.push(c);
            }
            Ok(out)
        }

        fn do_accept_pending(
            who: T::AccountId,
            asset: T::AssetId,
            deposits: Vec<u64>,
            accept_envelope: InputProof, // ΔC + 2 range proofs
        ) -> DispatchResult {
            let who_pk = PublicKey::<T>::get(&who).ok_or(Error::<T>::NoPublicKey)?;

            let avail_old_opt = AvailableBalanceCommit::<T>::get(asset, &who);
            let avail_old_buf;
            let avail_old: &[u8] = match avail_old_opt {
                Some(c) => {
                    avail_old_buf = c;
                    &avail_old_buf[..]
                }
                None => &[],
            };

            let pending_old_opt = PendingBalanceCommit::<T>::get(asset, &who);
            let pending_old_buf;
            let pending_old: &[u8] = match pending_old_opt {
                Some(c) => {
                    pending_old_buf = c;
                    &pending_old_buf[..]
                }
                None => &[],
            };

            let commits = Self::build_pending_commit_list(&who, &asset, &deposits)?;

            let (avail_new_raw, pending_new_raw) = T::Verifier::verify_transfer_received(
                &asset.using_encoded(|b| b.to_vec()),
                &who_pk,
                avail_old,
                pending_old,
                &commits,
                accept_envelope.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let avail_new = vec32(avail_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let pending_new = vec32(pending_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            for id in deposits {
                PendingDeposits::<T>::remove((who.clone(), asset, id));
            }

            AvailableBalanceCommit::<T>::insert(asset, &who, avail_new);
            if pending_new == [0u8; 32] {
                PendingBalanceCommit::<T>::remove(asset, &who);
            } else {
                PendingBalanceCommit::<T>::insert(asset, &who, pending_new);
            }

            Self::deposit_event(Event::PendingAccepted(who, asset));
            Ok(())
        }
    }

    // -------------------- Tiny util --------------------

    fn vec32(v: Vec<u8>) -> Result<[u8; 32], ()> {
        if v.len() != 32 {
            return Err(());
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&v);
        Ok(out)
    }
}
