#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

use fhe_primitives::{AlgoId, CtBlob, ParamsId, PubKeyBlob, ServerKeyRef};
use frame_support::{pallet_prelude::*, traits::ConstU32};
use frame_system::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Max ciphertext bytes allowed on-chain.
        #[pallet::constant]
        type MaxCiphertextBytes: Get<u32>;

        /// Max public key bytes allowed on-chain (if you store it).
        #[pallet::constant]
        type MaxPubKeyBytes: Get<u32>;

        /// Weight info (use () for now).
        type WeightInfo: WeightData;
    }

    /// Keep weights minimal for now.
    pub trait WeightData {
        fn request() -> Weight;
        fn set_pubkey() -> Weight;
        fn ingest() -> Weight;
    }
    impl WeightData for () {
        fn request() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn set_pubkey() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn ingest() -> Weight {
            Weight::from_parts(10_000, 0)
        }
    }

    /// Optional storage if you want to keep a registry of server keys (off-chain referenced).
    #[pallet::storage]
    pub type ServerKeyById<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], ServerKeyRef, OptionQuery>;

    /// Example storage for async results: context_id -> ciphertext
    #[pallet::storage]
    pub type ResultByContext<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], CtBlob, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Request emitted for off-chain coprocessor (FRAME event—your relayer can listen to these).
        FheAddRequested {
            who: T::AccountId,
            params: ParamsId,
            a_len: u32,
            b_len: u32,
            context: [u8; 32],
        },
        FheSubRequested {
            who: T::AccountId,
            params: ParamsId,
            a_len: u32,
            b_len: u32,
            context: [u8; 32],
        },
        FheCmpRequested {
            who: T::AccountId,
            params: ParamsId,
            a_len: u32,
            b_len: u32,
            context: [u8; 32],
        },
        /// Coprocessor posted the result.
        FheResultIngested { context: [u8; 32], params: ParamsId, len: u32 },
    }

    #[pallet::error]
    pub enum Error<T> {
        ResultAlreadyExists,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Request an ADD evaluation: emits Event that relayer/coprocessor can pick up.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::request())]
        pub fn request_add(
            origin: OriginFor<T>,
            params: ParamsId,
            ct_a: CtBlob,
            ct_b: CtBlob,
            context: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // size enforced by bounded input length
            Self::deposit_event(Event::FheAddRequested {
                who,
                params,
                a_len: ct_a.payload.len() as u32,
                b_len: ct_b.payload.len() as u32,
                context,
            });
            Ok(())
        }

        /// Request a SUB evaluation: emits Event that relayer/coprocessor can pick up.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::request())]
        pub fn request_sub(
            origin: OriginFor<T>,
            params: ParamsId,
            ct_a: CtBlob,
            ct_b: CtBlob,
            context: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // size enforced by bounded input length
            Self::deposit_event(Event::FheSubRequested {
                who,
                params,
                a_len: ct_a.payload.len() as u32,
                b_len: ct_b.payload.len() as u32,
                context,
            });
            Ok(())
        }

        /// Request an CMP evaluation: emits Event that relayer/coprocessor can pick up.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::request())]
        pub fn request_cmp(
            origin: OriginFor<T>,
            params: ParamsId,
            ct_a: CtBlob,
            ct_b: CtBlob,
            context: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // size enforced by bounded input length
            Self::deposit_event(Event::FheCmpRequested {
                who,
                params,
                a_len: ct_a.payload.len() as u32,
                b_len: ct_b.payload.len() as u32,
                context,
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_pubkey())]
        pub fn set_pubkey(origin: OriginFor<T>, pubkey: PubKeyBlob) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // TODO: associate PubkeyBlob with AccountId
            Ok(())
        }

        /// Coprocessor/relayer posts a result ciphertext tied to `context`.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::ingest())]
        pub fn ingest_result(
            origin: OriginFor<T>,
            params: ParamsId,
            context: [u8; 32],
            ct_out: CtBlob,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            // TODO: ensure associated with pubkey
            ensure!(ResultByContext::<T>::get(context).is_none(), Error::<T>::ResultAlreadyExists);
            let len = ct_out.payload.len() as u32;
            ResultByContext::<T>::insert(context, ct_out);
            Self::deposit_event(Event::FheResultIngested { context, params, len });
            Ok(())
        }
    }
}
