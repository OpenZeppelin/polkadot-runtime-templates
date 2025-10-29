// pallets/confidential-assets/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use confidential_assets_primitives::*;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, PalletId};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        /// Balance value type
        type Balance: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo + Default;

        /// Cryptographic backend implementing a encrypted balances store. Examples
        /// of backends used in practice may include ZK El Gamal, FHE, TEE.
        type Backend: ConfidentialBackend<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Plug in any ramp you want (naive now, Merkle/batched later).
        type Ramp: Ramp<Self::AccountId, Self::AssetId, Self::Balance>;

        /// PalletId to derive the custodial account used for holding escrowed
        /// balances iff `Self::Ramp` is implemented accordingly
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Optional ACL (default = ()).
        type Acl: AclProvider<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Operator layer. Defaults to always returning false when assigned ().
        type Operators: OperatorRegistry<Self::AccountId, Self::AssetId, BlockNumberFor<Self>>;

        type AssetMetadata: AssetMetadataProvider<Self::AssetId>;

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
        // On/Off Ramp Events (v0 without privacy)
        Deposited {
            who: T::AccountId,
            asset: T::AssetId,
            amount: T::Balance,
            encrypted_amount: EncryptedAmount,
        },
        Withdrawn {
            who: T::AccountId,
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
            amount: T::Balance,
        },
        // User calls
        PublicKeySet {
            who: T::AccountId,
        },
        ConfidentialTransfer {
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        ConfidentialClaimed {
            asset: T::AssetId,
            who: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        AmountDisclosed {
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
            amount: T::Balance,
            discloser: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        NotAuthorized,
        ReceiverRejected,
        BackendError,
        RampFailed,
        InsufficientConfidential, // if your debit fails
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
        /// User converts public -> confidential to shield assets
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_public_key())] //TODO
        pub fn deposit(
            origin: OriginFor<T>,
            asset: T::AssetId,
            amount: T::Balance,
            proof: InputProof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // pull public funds into pallet custody
            T::Ramp::burn(&who, &asset, amount).map_err(|_| Error::<T>::RampFailed)?;

            // credit confidential balance
            let encrypted_amount = T::Backend::mint_encrypted(asset, &who, amount, proof)?;

            Self::deposit_event(Event::Deposited {
                who,
                asset,
                amount,
                encrypted_amount,
            });
            Ok(())
        }

        /// User converts confidential -> public to unshield assets
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_public_key())] //TODO
        pub fn withdraw(
            origin: OriginFor<T>,
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // debit confidential (fail if insufficient)
            let amount = T::Backend::burn_encrypted(asset, &who, encrypted_amount, proof)
                .map_err(|_| Error::<T>::InsufficientConfidential)?;

            // push public funds out of pallet custody
            T::Ramp::mint(&who, &asset, amount).map_err(|_| Error::<T>::RampFailed)?;

            Self::deposit_event(Event::Withdrawn {
                who,
                asset,
                encrypted_amount,
                amount,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_public_key())]
        pub fn set_public_key(origin: OriginFor<T>, elgamal_pk: PublicKeyBytes) -> DispatchResult {
            let who = ensure_signed(origin)?;
            T::Backend::set_public_key(&who, &elgamal_pk).map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::PublicKeySet { who });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_transfer(
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

        #[pallet::call_index(4)]
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

        /// Allows users to accept pending deposits to make received confidential
        /// balances available to transfer. TODO: link to longer explanation
        // TODO: consider exposing confidential_claim_and_transfer aka pallet_zkhe::accept_pending_and_transfer
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())] // TODO
        pub fn confidential_claim(
            origin: OriginFor<T>,
            asset: T::AssetId,
            input_proof: InputProof,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let claimed = T::Backend::claim_encrypted(asset, &from, input_proof)
                .map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::ConfidentialClaimed {
                asset,
                who: from,
                encrypted_amount: claimed,
            });
            Ok(())
        }

        // ---------- Operator + ACL permutations for all extrinsics ----------

        /// Operator and/or ACL-driven confidential transfer
        // TODO: impl for deposit, withraw, claim(accept_pending), etc
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from())]
        pub fn confidential_transfer_from(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::ensure_is_self_or_operator(&from, &asset, &caller)?;
            T::Acl::authorize(
                Op::Transfer,
                &AclCtx {
                    amount: Default::default(),
                    asset,
                    caller: caller.clone(),
                    owner: Some(from.clone()),
                    counterparty: Some(to.clone()),
                    opaque: sp_std::vec![],
                },
            )?;
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

        /// ACL-driven confidential transfer
        // TODO: for prod impl for deposit, withraw, claim(accept_pending)
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from())]
        pub fn confidential_transfer_acl(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
            input_proof: InputProof,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            T::Acl::authorize(
                Op::Transfer,
                &AclCtx {
                    amount: Default::default(),
                    asset,
                    caller: caller.clone(),
                    owner: Some(from.clone()),
                    counterparty: Some(to.clone()),
                    opaque: sp_std::vec![],
                },
            )?;
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
    }

    impl<T: Config> Pallet<T> {
        /// Pallet custodial account.
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }
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
