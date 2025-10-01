//! FHE Oracle Pallet for requesting/fulfilling FHE computation
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{pallet_prelude::*, traits::EnsureOrigin, BoundedVec};
    use frame_system::pallet_prelude::*;
    use oz_fhe_primitives::oracle::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Origin allowed to fulfill requests.
        type FulfillOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Optional hook for business logic on fulfillment.
        type OnFulfill: OracleFulfillHook<Self>;
        /// Max bytes for payload/result.
        #[pallet::constant]
        type MaxBlobLen: Get<u32>;
        /// Weight info
        type WeightInfo: WeightData;
    }

    pub trait WeightData {
        fn request() -> Weight;
        fn fulfill() -> Weight;
        fn fail() -> Weight;
    }
    impl WeightData for () {
        fn request() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn fulfill() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn fail() -> Weight {
            Weight::from_parts(10_000, 0)
        }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type NextRequestId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    pub type Requests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        Request<T::AccountId, BoundedVec<u8, T::MaxBlobLen>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Requested {
            id: u64,
            who: T::AccountId,
            purpose: Purpose,
        },
        Fulfilled {
            id: u64,
        },
        Failed {
            id: u64,
            reason: Vec<u8>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        UnknownRequest,
        AlreadyCompleted,
        NotPending,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Anyone can request (enqueue) a job.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::request())]
        pub fn request(
            origin: OriginFor<T>,
            payload: BoundedVec<u8, T::MaxBlobLen>,
            purpose: Purpose,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let id = NextRequestId::<T>::mutate(|n| {
                let id = *n;
                *n += 1;
                id
            });
            let req = Request {
                who: who.clone(),
                purpose,
                payload,
                status: Status::Pending,
            };
            Requests::<T>::insert(id, req);
            Self::deposit_event(Event::Requested { id, who, purpose });
            Ok(())
        }

        /// Trusted fulfiller posts the result (plaintext or re-encrypted-to-policy).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::fulfill())]
        pub fn fulfill(
            origin: OriginFor<T>,
            id: u64,
            result: BoundedVec<u8, T::MaxBlobLen>,
        ) -> DispatchResult {
            T::FulfillOrigin::ensure_origin(origin)?;
            Requests::<T>::try_mutate(id, |maybe| -> DispatchResult {
                let req = maybe.as_mut().ok_or(Error::<T>::UnknownRequest)?;
                ensure!(
                    matches!(req.status, Status::Pending),
                    Error::<T>::NotPending
                );
                req.status = Status::Completed;
                let (who, purpose, payload) = (req.who.clone(), req.purpose, req.payload.clone());
                // Call the hook (business logic continues here)
                T::OnFulfill::on_fulfill(id, &who, purpose, &payload, &result);
                Ok(())
            })?;
            Self::deposit_event(Event::Fulfilled { id });
            Ok(())
        }

        /// Mark as failed with a reason (optional).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::fail())]
        pub fn fail(
            origin: OriginFor<T>,
            id: u64,
            reason: BoundedVec<u8, T::MaxBlobLen>,
        ) -> DispatchResult {
            T::FulfillOrigin::ensure_origin(origin)?;
            Requests::<T>::try_mutate(id, |maybe| -> DispatchResult {
                let req = maybe.as_mut().ok_or(Error::<T>::UnknownRequest)?;
                ensure!(
                    matches!(req.status, Status::Pending),
                    Error::<T>::NotPending
                );
                req.status = Status::Failed;
                Ok(())
            })?;
            Self::deposit_event(Event::Failed {
                id,
                reason: reason.to_vec(),
            });
            Ok(())
        }
    }

    // Off-chain/indexed access.
    impl<T: Config> Pallet<T> {
        pub fn get_request(
            id: u64,
        ) -> Option<Request<T::AccountId, BoundedVec<u8, T::MaxBlobLen>>> {
            Requests::<T>::get(id)
        }
    }
}
