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
        type Balance: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo + From<u64>;

        /// Verifier boundary (no_std on-chain).
        /// - verify_transfer_sent(..) -> (from_new_commit, to_new_pending_commit)
        /// - verify_transfer_received(.., pending_commits: &[[u8;32]], accept_envelope: &[u8])
        /// - verify_mint(..) -> (to_new_pending_commit, total_new_commit, minted_ciphertext)
        /// - verify_burn(..) -> (from_new_available_commit, total_new_commit, disclosed_amount_u64)
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
        Transferred {
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        PendingAccepted {
            asset: T::AssetId,
            who: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        PendingAcceptedAndTransferred {
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        NoPublicKey,
        InvalidProof,
        BackendPolicy,
        BadCipher,
        NoPending,
        SupplyMismatch,
        MalformedEnvelope,
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
            encrypted_amount: EncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred = Self::transfer_encrypted(asset, &from, &to, encrypted_amount, proof)?;
            Self::deposit_event(Event::Transferred {
                asset,
                from,
                to,
                encrypted_amount: transferred,
            });
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
            accept_envelope: InputProof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let claimed = Self::claim_encrypted(asset, &who, accept_envelope)?;
            Self::deposit_event(Event::PendingAccepted {
                asset,
                who,
                encrypted_amount: claimed,
            });
            Ok(())
        }

        /// Accept pending then transfer from available.
        /// Enables spend of pending deposits in one transaction.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::transfer_from_available())]
        pub fn accept_pending_and_transfer(
            origin: T::RuntimeOrigin,
            asset: T::AssetId,
            to: T::AccountId,
            accept_envelope: InputProof,
            transfer_proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let claimed = Self::claim_encrypted(asset, &from, accept_envelope)?;
            let transferred = Self::transfer_encrypted(asset, &from, &to, claimed, transfer_proof)?;
            Self::deposit_event(Event::PendingAcceptedAndTransferred {
                asset,
                from,
                to,
                encrypted_amount: transferred,
            });
            Ok(())
        }
    }

    impl<T: Config> ConfidentialBackend<T::AccountId, T::AssetId, T::Balance> for Pallet<T> {
        fn set_public_key(
            who: &T::AccountId,
            elgamal_pk: &PublicKeyBytes,
        ) -> Result<(), DispatchError> {
            ensure!(!elgamal_pk.is_empty(), Error::<T>::BadCipher);
            PublicKey::<T>::insert(who, elgamal_pk.clone());
            Ok(())
        }

        fn total_supply(asset: T::AssetId) -> [u8; 32] {
            TotalSupplyCommit::<T>::get(asset).unwrap_or([0u8; 32])
        }

        fn balance_of(asset: T::AssetId, who: &T::AccountId) -> [u8; 32] {
            AvailableBalanceCommit::<T>::get(asset, who).unwrap_or([0u8; 32])
        }

        fn disclose_amount(
            asset: T::AssetId,
            encrypted_amount: &EncryptedAmount,
            who: &T::AccountId,
        ) -> Result<T::Balance, DispatchError> {
            let pk = PublicKey::<T>::get(who).ok_or(Error::<T>::NoPublicKey)?;
            let amount = T::Verifier::disclose(
                &asset.using_encoded(|b| b.to_vec()),
                &pk,
                &encrypted_amount[..],
            )
            .map_err(|_| Error::<T>::BackendPolicy)?;
            Ok(amount.into())
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

        fn claim_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            // Thin wrapper around accept_pending:
            // input_proof is assumed to be:
            //   count:u16 || ids[count]*u64 || accept_envelope:bytes
            let (ids, accept_envelope) = Self::parse_ids_and_accept_envelope(&input_proof)
                .map_err(|_| Error::<T>::MalformedEnvelope)?;

            // Perform the same logic as accept_pending for `from`.
            Self::do_accept_pending(from.clone(), asset, ids, accept_envelope)?;

            // Interface returns an EncryptedAmount; for a pure "claim"/"unlock" there is no new
            // ciphertext to return. Return 64 zero bytes to signal "no new UTXO created".
            Ok([0u8; 64])
        }

        fn mint_encrypted(
            asset: T::AssetId,
            to: &T::AccountId,
            amount: T::Balance,
            input_proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError> {
            // KISS mint path:
            // - verify_mint proves: pending(to) += v, total_supply(asset) += v
            // - it also returns the freshly minted ciphertext for the recipient UTXO list
            let to_pk = PublicKey::<T>::get(to).ok_or(Error::<T>::NoPublicKey)?;

            let to_old_pending_opt = PendingBalanceCommit::<T>::get(asset, to);
            let to_old_pending_buf;
            let to_old_pending: &[u8] = match to_old_pending_opt {
                Some(c) => {
                    to_old_pending_buf = c;
                    &to_old_pending_buf[..]
                }
                None => &[],
            };

            let total_old_opt = TotalSupplyCommit::<T>::get(asset);
            let total_old_buf;
            let total_old: &[u8] = match total_old_opt {
                Some(c) => {
                    total_old_buf = c;
                    &total_old_buf[..]
                }
                None => &[],
            };

            let (to_new_pending_raw, total_new_raw, minted_ct) = T::Verifier::verify_mint(
                &asset.using_encoded(|b| b.to_vec()),
                &to_pk,
                to_old_pending,
                total_old,
                &amount.using_encoded(|b| b.to_vec()),
                input_proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let to_new_pending = vec32(to_new_pending_raw).map_err(|_| Error::<T>::BadCipher)?;
            let total_new = vec32(total_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            // Update storage
            PendingBalanceCommit::<T>::insert(asset, to, to_new_pending);
            TotalSupplyCommit::<T>::insert(asset, total_new);

            // Record the minted UTXO for `to`
            let id = NextPendingDepositId::<T>::get(to, &asset);
            PendingDeposits::<T>::insert((to, asset, id), minted_ct);
            NextPendingDepositId::<T>::insert(to, asset, id + 1);

            Ok(minted_ct)
        }

        fn burn_encrypted(
            asset: T::AssetId,
            from: &T::AccountId,
            amount_ciphertext: EncryptedAmount,
            input_proof: InputProof,
        ) -> Result<T::Balance, DispatchError> {
            // KISS burn path:
            // - verify_burn proves: available(from) -= v, total_supply(asset) -= v,
            //   and that `amount_ciphertext` indeed encrypts v under `from`'s key (or policy key).
            // - it returns new commits and the disclosed v (u64 -> T::Balance).
            let from_pk = PublicKey::<T>::get(from).ok_or(Error::<T>::NoPublicKey)?;

            let from_old_avail_opt = AvailableBalanceCommit::<T>::get(asset, from);
            let from_old_avail_buf;
            let from_old_avail: &[u8] = match from_old_avail_opt {
                Some(c) => {
                    from_old_avail_buf = c;
                    &from_old_avail_buf[..]
                }
                None => &[],
            };

            let total_old_opt = TotalSupplyCommit::<T>::get(asset);
            let total_old_buf;
            let total_old: &[u8] = match total_old_opt {
                Some(c) => {
                    total_old_buf = c;
                    &total_old_buf[..]
                }
                None => &[],
            };

            let (from_new_raw, total_new_raw, disclosed_u64) = T::Verifier::verify_burn(
                &asset.using_encoded(|b| b.to_vec()),
                &from_pk,
                from_old_avail,
                total_old,
                &amount_ciphertext,
                input_proof.as_slice(),
            )
            .map_err(|_| Error::<T>::InvalidProof)?;

            let from_new = vec32(from_new_raw).map_err(|_| Error::<T>::BadCipher)?;
            let total_new = vec32(total_new_raw).map_err(|_| Error::<T>::BadCipher)?;

            AvailableBalanceCommit::<T>::insert(asset, from, from_new);
            TotalSupplyCommit::<T>::insert(asset, total_new);

            Ok(disclosed_u64.into())
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

        /// Parse `[ids]*` + envelope from a single `InputProof`.
        /// Layout:
        ///   - u16: count
        ///   - count * u64: deposit ids (LE)
        ///   - remaining bytes: accept envelope (opaque for verifier)
        fn parse_ids_and_accept_envelope(input: &InputProof) -> Result<(Vec<u64>, InputProof), ()> {
            let bytes = input.as_slice();
            if bytes.len() < 2 {
                return Err(());
            }
            let count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
            let need = 2 + count * 8;
            if bytes.len() < need {
                return Err(());
            }
            let mut ids = Vec::with_capacity(count);
            let mut off = 2;
            for _ in 0..count {
                let mut le = [0u8; 8];
                le.copy_from_slice(&bytes[off..off + 8]);
                ids.push(u64::from_le_bytes(le));
                off += 8;
            }
            let rest = &bytes[off..];
            // Re-wrap remainder into InputProof
            let env: InputProof = rest.to_vec().try_into().map_err(|_| ())?;
            Ok((ids, env))
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
