// pallets/confidential-assets/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use confidential_assets_primitives::*;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        type Backend: ConfidentialBackend<Self::AccountId, Self::AssetId>;

        type AssetMetadata: AssetMetadataProvider<Self::AssetId>;

        type OnConfidentialTransfer: OnConfidentialTransfer<Self::AccountId, Self::AssetId>;

        /// OPTIONAL operator layer. Defaults to no-ops (always false).
        type Operators: OperatorRegistry<Self::AccountId, Self::AssetId, BlockNumberFor<Self>>;

        type WeightInfo: WeightData;
    }

    pub trait WeightData {
        fn set_public_key() -> Weight;
        fn confidential_transfer() -> Weight;
        fn confidential_transfer_from() -> Weight;
        fn confidential_transfer_and_call() -> Weight;
        fn confidential_transfer_from_and_call() -> Weight;
        fn disclose_amount() -> Weight;
    }
    impl WeightData for () {
        fn set_public_key() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn confidential_transfer() -> Weight {
            Weight::from_parts(20_000, 0)
        }
        fn confidential_transfer_from() -> Weight {
            Weight::from_parts(22_000, 0)
        }
        fn confidential_transfer_and_call() -> Weight {
            Weight::from_parts(25_000, 0)
        }
        fn confidential_transfer_from_and_call() -> Weight {
            Weight::from_parts(27_000, 0)
        }
        fn disclose_amount() -> Weight {
            Weight::from_parts(5_000, 0)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ConfidentialTransfer {
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        AmountDisclosed {
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
            amount: u64,
            discloser: T::AccountId,
        },
        PublicKeySet {
            who: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        NotAuthorized,
        ReceiverRejected,
        BackendError,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ---------- Read helpers ----------
    impl<T: Config> Pallet<T> {
        pub fn confidential_total_supply(asset: T::AssetId) -> Commitment {
            T::Backend::total_supply(asset)
        }
        pub fn confidential_balance_of(asset: T::AssetId, who: &T::AccountId) -> Commitment {
            T::Backend::balance_of(asset, who)
        }
        pub fn asset_name(asset: T::AssetId) -> Vec<u8> {
            T::AssetMetadata::name(asset)
        }
        pub fn asset_symbol(asset: T::AssetId) -> Vec<u8> {
            T::AssetMetadata::symbol(asset)
        }
        pub fn asset_decimals(asset: T::AssetId) -> u8 {
            T::AssetMetadata::decimals(asset)
        }
        pub fn asset_contract_uri(asset: T::AssetId) -> Vec<u8> {
            T::AssetMetadata::contract_uri(asset)
        }
    }

    // ---------- Calls ----------
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_public_key())]
        pub fn set_public_key(origin: OriginFor<T>, elgamal_pk: PublicKeyBytes) -> DispatchResult {
            let who = ensure_signed(origin)?;
            T::Backend::set_public_key(&who, &elgamal_pk).map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::PublicKeySet { who });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_transfer_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred =
                T::Backend::transfer_encrypted(asset, &from, &to, encrypted_amount, input_proof)
                    .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_transfer_acl(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            amount: EncryptedAmount,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred = T::Backend::transfer_acl(asset, &from, &to, amount)
                .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from())]
        pub fn confidential_transfer_from_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::ensure_is_self_or_operator(&from, &asset, &caller)?;
            let transferred =
                T::Backend::transfer_encrypted(asset, &from, &to, encrypted_amount, input_proof)
                    .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from())]
        pub fn confidential_transfer_from_acl(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            amount: EncryptedAmount,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::ensure_is_self_or_operator(&from, &asset, &caller)?;
            let transferred = T::Backend::transfer_acl(asset, &from, &to, amount)
                .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_and_call())]
        pub fn confidential_transfer_and_call_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
            data: CallbackData,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred =
                T::Backend::transfer_encrypted(asset, &from, &to, encrypted_amount, input_proof)
                    .map_err(|_| Error::<T>::BackendError)?;
            T::OnConfidentialTransfer::on_confidential_transfer_received(
                &from,
                &to,
                &asset,
                &transferred,
                &data,
            )
            .map_err(|_| Error::<T>::ReceiverRejected)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_and_call())]
        pub fn confidential_transfer_and_call_acl(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            amount: EncryptedAmount,
            data: CallbackData,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred = T::Backend::transfer_acl(asset, &from, &to, amount)
                .map_err(|_| Error::<T>::BackendError)?;
            T::OnConfidentialTransfer::on_confidential_transfer_received(
                &from,
                &to,
                &asset,
                &transferred,
                &data,
            )
            .map_err(|_| Error::<T>::ReceiverRejected)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from_and_call())]
        pub fn confidential_transfer_from_and_call_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
            data: CallbackData,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::ensure_is_self_or_operator(&from, &asset, &caller)?;
            let transferred =
                T::Backend::transfer_encrypted(asset, &from, &to, encrypted_amount, input_proof)
                    .map_err(|_| Error::<T>::BackendError)?;
            T::OnConfidentialTransfer::on_confidential_transfer_received(
                &from,
                &to,
                &asset,
                &transferred,
                &data,
            )
            .map_err(|_| Error::<T>::ReceiverRejected)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from_and_call())]
        pub fn confidential_transfer_from_and_call_acl(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            amount: EncryptedAmount,
            data: CallbackData,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::ensure_is_self_or_operator(&from, &asset, &caller)?;
            let transferred = T::Backend::transfer_acl(asset, &from, &to, amount)
                .map_err(|_| Error::<T>::BackendError)?;
            T::OnConfidentialTransfer::on_confidential_transfer_received(
                &from,
                &to,
                &asset,
                &transferred,
                &data,
            )
            .map_err(|_| Error::<T>::ReceiverRejected)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from: from.clone(),
                to: to.clone(),
                encrypted_amount: transferred.clone(),
            });
            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::disclose_amount())]
        pub fn disclose_amount(
            origin: OriginFor<T>,
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let amount = T::Backend::disclose_amount(asset, &encrypted_amount, &who)
                .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::AmountDisclosed {
                asset,
                encrypted_amount,
                amount,
                discloser: who,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        #[inline]
        fn ensure_is_self_or_operator(
            holder: &T::AccountId,
            asset: &T::AssetId,
            caller: &T::AccountId,
        ) -> Result<(), Error<T>> {
            if caller == holder {
                return Ok(());
            }
            let now = <frame_system::Pallet<T>>::block_number();
            if T::Operators::is_operator(holder, asset, caller, now) {
                Ok(())
            } else {
                Err(Error::<T>::NotAuthorized)
            }
        }
    }
}
