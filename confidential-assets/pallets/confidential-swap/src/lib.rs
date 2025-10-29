// pallets/confidential-swaps/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame_support::{dispatch::DispatchResult, pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use confidential_assets_primitives::{ConfidentialBackend, EncryptedAmount, InputProof, Ramp};

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Optional commitment to taker-leg terms (KISS: blake2 hash of (asset_b, b_to_a_ct)).
    /// If all zeros, any taker leg is accepted.
    #[derive(
        Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default, RuntimeDebug,
    )]
    pub struct TermsHash(pub [u8; 32]);

    /// Confidential↔Confidential maker intent.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct SwapIntentCc<AccountId, AssetId> {
        pub proposer: AccountId,
        pub counterparty: AccountId,
        pub asset_a: AssetId,           // maker sends on A
        pub asset_b: AssetId,           // taker sends on B
        pub a_to_b_ct: EncryptedAmount, // maker ciphertext (A -> counterparty)
        pub a_to_b_proof: InputProof,   // maker proof
        pub terms_hash: TermsHash,      // optional predicate binding taker leg
    }

    /// Confidential→Public maker intent (maker sends confidential, expects public).
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub struct SwapIntentCp<AccountId, AssetId, Balance> {
        pub proposer: AccountId,
        pub counterparty: AccountId,
        pub asset_conf: AssetId,        // maker sends on confidential side
        pub asset_public: AssetId,      // taker pays on public side
        pub a_to_b_ct: EncryptedAmount, // maker ciphertext to taker
        pub a_to_b_proof: InputProof,   // maker proof
        pub amount_public: Balance,     // exact public amount the taker must pay
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;
        type Balance: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo + Default;

        type Backend: ConfidentialBackend<Self::AccountId, Self::AssetId, Self::Balance>;
        type Ramp: Ramp<Self::AccountId, Self::AssetId, Self::Balance>;

        type WeightInfo: WeightInfo;
    }

    pub trait WeightInfo {
        fn open_cc() -> Weight;
        fn cancel_cc() -> Weight;
        fn accept_cc() -> Weight;
        fn open_cp() -> Weight;
        fn cancel_cp() -> Weight;
        fn accept_cp() -> Weight;
    }

    impl WeightInfo for () {
        fn open_cc() -> Weight {
            10_000.into()
        }
        fn cancel_cc() -> Weight {
            5_000.into()
        }
        fn accept_cc() -> Weight {
            25_000.into()
        }
        fn open_cp() -> Weight {
            10_000.into()
        }
        fn cancel_cp() -> Weight {
            5_000.into()
        }
        fn accept_cp() -> Weight {
            25_000.into()
        }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ---- Storage ----
    #[pallet::storage]
    #[pallet::getter(fn next_cc_id)]
    pub type NextCcId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn cc_swaps)]
    pub type CcSwaps<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, SwapIntentCc<T::AccountId, T::AssetId>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_cp_id)]
    pub type NextCpId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn cp_swaps)]
    pub type CpSwaps<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        SwapIntentCp<T::AccountId, T::AssetId, T::Balance>,
        OptionQuery,
    >;

    // ---- Events / Errors ----
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CcOpened {
            id: u64,
            proposer: T::AccountId,
            counterparty: T::AccountId,
            asset_a: T::AssetId,
            asset_b: T::AssetId,
        },
        CcCanceled {
            id: u64,
            proposer: T::AccountId,
        },
        CcExecuted {
            id: u64,
            proposer: T::AccountId,
            counterparty: T::AccountId,
        },

        CpOpened {
            id: u64,
            proposer: T::AccountId,
            counterparty: T::AccountId,
            asset_conf: T::AssetId,
            asset_public: T::AssetId,
            amount_public: T::Balance,
        },
        CpCanceled {
            id: u64,
            proposer: T::AccountId,
        },
        CpExecuted {
            id: u64,
            proposer: T::AccountId,
            counterparty: T::AccountId,
            amount_public: T::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        UnknownSwap,
        NotProposer,
        NotCounterparty,
        TermsMismatch, // taker leg did not match maker's hash predicate
        BackendError,
        RampFailed,
    }

    // ---- Calls ----
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // --------- Confidential <-> Confidential ---------

        /// Maker opens a C↔C intent, optionally binding taker leg with a terms hash.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::open_cc())]
        pub fn open_swap_cc(
            origin: OriginFor<T>,
            counterparty: T::AccountId,
            asset_a: T::AssetId,
            asset_b: T::AssetId,
            a_to_b_ct: EncryptedAmount,
            a_to_b_proof: InputProof,
            terms_hash: Option<[u8; 32]>, // None -> accept any taker ciphertext on (asset_b)
        ) -> DispatchResult {
            let proposer = ensure_signed(origin)?;

            let id = NextCcId::<T>::mutate(|n| {
                let cur = *n;
                *n = n.saturating_add(1);
                cur
            });
            let th = TermsHash(terms_hash.unwrap_or([0u8; 32]));
            CcSwaps::<T>::insert(
                id,
                SwapIntentCc {
                    proposer: proposer.clone(),
                    counterparty: counterparty.clone(),
                    asset_a,
                    asset_b,
                    a_to_b_ct,
                    a_to_b_proof,
                    terms_hash: th,
                },
            );

            Self::deposit_event(Event::CcOpened {
                id,
                proposer,
                counterparty,
                asset_a,
                asset_b,
            });
            Ok(())
        }

        /// Cancel a C↔C intent (maker only).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::cancel_cc())]
        pub fn cancel_swap_cc(origin: OriginFor<T>, id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let intent = CcSwaps::<T>::take(id).ok_or(Error::<T>::UnknownSwap)?;
            ensure!(intent.proposer == who, Error::<T>::NotProposer);
            Self::deposit_event(Event::CcCanceled { id, proposer: who });
            Ok(())
        }

        /// Accept and atomically execute a C↔C swap.
        ///
        /// Atomicity is enforced with FRAME's transactional storage layer.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::accept_cc())]
        #[transactional]
        pub fn accept_swap_cc(
            origin: OriginFor<T>,
            id: u64,
            b_to_a_ct: EncryptedAmount,
            b_to_a_proof: InputProof,
        ) -> DispatchResult {
            let counterparty = ensure_signed(origin)?;
            let intent = CcSwaps::<T>::take(id).ok_or(Error::<T>::UnknownSwap)?;
            ensure!(
                intent.counterparty == counterparty,
                Error::<T>::NotCounterparty
            );

            // Optional predicate: bind taker leg (asset_b, b_to_a_ct).
            if intent.terms_hash.0 != [0u8; 32] {
                let mut enc = intent.asset_b.encode();
                enc.extend_from_slice(&b_to_a_ct);
                let h = sp_io::hashing::blake2_256(&enc);
                ensure!(h == intent.terms_hash.0, Error::<T>::TermsMismatch);
            }

            // Leg 1: proposer -> counterparty on asset_a
            T::Backend::transfer_encrypted(
                intent.asset_a,
                &intent.proposer,
                &counterparty,
                intent.a_to_b_ct,
                intent.a_to_b_proof,
            )
            .map_err(|_| Error::<T>::BackendError)?;

            // Leg 2: counterparty -> proposer on asset_b
            T::Backend::transfer_encrypted(
                intent.asset_b,
                &counterparty,
                &intent.proposer,
                b_to_a_ct,
                b_to_a_proof,
            )
            .map_err(|_| Error::<T>::BackendError)?;

            Self::deposit_event(Event::CcExecuted {
                id,
                proposer: intent.proposer,
                counterparty,
            });
            Ok(())
        }

        // --------- Confidential -> Public ---------

        /// Maker opens a C→P intent: send confidential to taker and receive an exact public amount.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::open_cp())]
        pub fn open_swap_cp(
            origin: OriginFor<T>,
            counterparty: T::AccountId,
            asset_conf: T::AssetId,
            asset_public: T::AssetId,
            a_to_b_ct: EncryptedAmount,
            a_to_b_proof: InputProof,
            amount_public: T::Balance,
        ) -> DispatchResult {
            let proposer = ensure_signed(origin)?;

            let id = NextCpId::<T>::mutate(|n| {
                let cur = *n;
                *n = n.saturating_add(1);
                cur
            });
            CpSwaps::<T>::insert(
                id,
                SwapIntentCp {
                    proposer: proposer.clone(),
                    counterparty: counterparty.clone(),
                    asset_conf,
                    asset_public,
                    a_to_b_ct,
                    a_to_b_proof,
                    amount_public,
                },
            );

            Self::deposit_event(Event::CpOpened {
                id,
                proposer,
                counterparty,
                asset_conf,
                asset_public,
                amount_public,
            });
            Ok(())
        }

        /// Cancel a C→P intent (maker only).
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::cancel_cp())]
        pub fn cancel_swap_cp(origin: OriginFor<T>, id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let intent = CpSwaps::<T>::take(id).ok_or(Error::<T>::UnknownSwap)?;
            ensure!(intent.proposer == who, Error::<T>::NotProposer);
            Self::deposit_event(Event::CpCanceled { id, proposer: who });
            Ok(())
        }

        /// Accept and atomically execute a C→P swap.
        ///
        /// Leg 1: confidential transfer (maker → taker).  
        /// Leg 2: public transfer_from (taker → maker) for the exact `amount_public`.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::accept_cp())]
        #[transactional]
        pub fn accept_swap_cp(origin: OriginFor<T>, id: u64) -> DispatchResult {
            let counterparty = ensure_signed(origin)?;
            let intent = CpSwaps::<T>::take(id).ok_or(Error::<T>::UnknownSwap)?;
            ensure!(
                intent.counterparty == counterparty,
                Error::<T>::NotCounterparty
            );

            // Confidential leg
            T::Backend::transfer_encrypted(
                intent.asset_conf,
                &intent.proposer,
                &counterparty,
                intent.a_to_b_ct,
                intent.a_to_b_proof,
            )
            .map_err(|_| Error::<T>::BackendError)?;

            // Public leg (escrowless): taker pays maker directly.
            T::Ramp::transfer_from(
                &counterparty,
                &intent.proposer,
                intent.asset_public,
                intent.amount_public,
            )
            .map_err(|_| Error::<T>::RampFailed)?;

            Self::deposit_event(Event::CpExecuted {
                id,
                proposer: intent.proposer,
                counterparty,
                amount_public: intent.amount_public,
            });
            Ok(())
        }
    }
}
