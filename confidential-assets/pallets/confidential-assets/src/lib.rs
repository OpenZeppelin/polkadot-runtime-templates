#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-confidential-assets (Multi-Asset, IERC-7984-inspired, Zether-backed)
//!
//! User-facing pallet that exposes a **confidential fungible token surface** per `AssetId`,
//! delegating verification and state updates to an abstract **backend** (here, Zether style).
//!
//! ## Mirrors of IERC-7984 (adapted to Substrate + multi-asset)
//! - `setOperator(holder->operator, until)` → per (holder, asset, operator) with block expiry
//! - `confidentialTransfer(to, encryptedAmount, inputProof)` → `confidential_transfer_encrypted`
//! - `confidentialTransfer(to, amount)` (no proof; ACL-gated) → `confidential_transfer_acl`
//! - `confidentialTransferFrom(from, to, encryptedAmount, inputProof)`
//! - `confidentialTransferFrom(from, to, amount)` (no proof; ACL-gated)
//! - `*_and_call` variants call a runtime hook `OnConfidentialTransfer` with `(from, to, asset, transferred, data)`
//! - Read helpers: `confidential_total_supply`, `confidential_balance_of`, `is_operator`
//! - **Key management (Solana/Zether-like):** `set_public_key` writes the ElGamal/curve pk via backend
//!
//! ## Layers
//! - **pallet-zether** → holds public keys, opaque commitments/ciphertexts, and verifies proofs.
//! - **pallet-confidential-assets** → operators, transfer surface, metadata proxy, receiver hooks.
//!
//! ## Opaque types & ACL
//! - Encrypted amount types are opaque bytes (to keep no_std & crypto-lib-agnostic).
//! - "ACL transfers" (no proof) must be authorized/validated by the backend.
//!
//! ## You implement (in your backend pallet, e.g., pallet-zether):
//! - `backend::ConfidentialBackend` (totals, balances, key mgmt, transfer(encrypted/acl), disclose)
//!
// TODO: on/off ramp functionality

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
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type used across the stack.
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        /// Backend that verifies & applies confidential ops (Zether).
        type Backend: ConfidentialBackend<Self::AccountId, Self::AssetId>;

        /// Asset metadata provider (name/symbol/decimals/contract_uri).
        type AssetMetadata: AssetMetadataProvider<Self::AssetId>;

        /// Post-transfer receiver callback (no-op by default).
        type OnConfidentialTransfer: OnConfidentialTransfer<Self::AccountId, Self::AssetId>;

        /// Weight info
        type WeightInfo: WeightData;
    }

    /// Minimal weight trait; replace with real benches later.
    pub trait WeightData {
        fn set_public_key() -> Weight;
        fn set_operator() -> Weight;
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
        fn set_operator() -> Weight {
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

    /// Storage for operators: (holder, asset, operator) -> until_block
    #[pallet::storage]
    pub type Operators<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>, // holder
            NMapKey<Blake2_128Concat, T::AssetId>,   // asset
            NMapKey<Blake2_128Concat, T::AccountId>, // operator
        ),
        BlockNumberFor<T>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Mirrors IERC-7984 OperatorSet, extended with asset id.
        OperatorSet {
            asset: T::AssetId,
            holder: T::AccountId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        },
        /// A confidential transfer occurred (encrypted amount).
        ConfidentialTransfer {
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: EncryptedAmount,
        },
        /// Disclosure occurred (implementation-specific policy).
        AmountDisclosed {
            asset: T::AssetId,
            encrypted_amount: EncryptedAmount,
            amount: u64,
            discloser: T::AccountId,
        },
        /// Per-account public key was set/updated.
        PublicKeySet { who: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        NotAuthorized,
        OperatorExpired,
        ReceiverRejected,
        BackendError,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ------------------------------
    // Helper queries (read-style)
    // ------------------------------

    impl<T: Config> Pallet<T> {
        /// Mirrors `IERC7984.confidentialTotalSupply()` but with AssetId.
        pub fn confidential_total_supply(asset: T::AssetId) -> EncryptedAmount {
            T::Backend::total_supply(asset)
        }

        /// Mirrors `IERC7984.confidentialBalanceOf(account)` but with AssetId.
        pub fn confidential_balance_of(asset: T::AssetId, who: &T::AccountId) -> EncryptedAmount {
            T::Backend::balance_of(asset, who)
        }

        /// Mirrors `IERC7984.isOperator(holder, spender)` but with AssetId.
        pub fn is_operator(
            holder: &T::AccountId,
            asset: &T::AssetId,
            spender: &T::AccountId,
        ) -> bool {
            let now = <frame_system::Pallet<T>>::block_number();
            match Operators::<T>::get((holder, asset, spender)) {
                Some(until) => now <= until,
                None => false,
            }
        }

        // Metadata pass-through
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

    // ------------------------------
    // Calls (only public API you expose)
    // ------------------------------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register or update the caller's Zether/Solana-like ElGamal public key in the backend.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_public_key())]
        pub fn set_public_key(origin: OriginFor<T>, elgamal_pk: PublicKeyBytes) -> DispatchResult {
            let who = ensure_signed(origin)?;
            T::Backend::set_public_key(&who, &elgamal_pk).map_err(|_| Error::<T>::BackendError)?;
            Self::deposit_event(Event::PublicKeySet { who });
            Ok(())
        }

        /// Set/extend an operator for a holder & asset (no checks on `until` at write time).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_operator())]
        pub fn set_operator(
            origin: OriginFor<T>,
            asset: T::AssetId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            Operators::<T>::insert((holder.clone(), asset, operator.clone()), until);
            Self::deposit_event(Event::OperatorSet {
                asset,
                holder,
                operator,
                until,
            });
            Ok(())
        }

        /// `confidentialTransfer(to, encryptedAmount, inputProof)` style.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_transfer_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
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

        /// `confidentialTransfer(to, amount)` (no proof; requires backend ACL).
        #[pallet::call_index(3)]
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

        /// `confidentialTransferFrom(from, to, encryptedAmount, inputProof)` with operator check.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from())]
        pub fn confidential_transfer_from_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
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

        /// `confidentialTransferFrom(from, to, amount)` (no proof; requires backend ACL) with operator check.
        #[pallet::call_index(5)]
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

        /// `confidentialTransferAndCall` (encrypted + proof) with receiver hook.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_and_call())]
        pub fn confidential_transfer_and_call_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
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

        /// `confidentialTransferAndCall` (ACL) with receiver hook.
        #[pallet::call_index(7)]
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

        /// `confidentialTransferFromAndCall` (encrypted + proof).
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::confidential_transfer_from_and_call())]
        pub fn confidential_transfer_from_and_call_encrypted(
            origin: OriginFor<T>,
            asset: T::AssetId,
            from: T::AccountId,
            to: T::AccountId,
            encrypted_amount: ExternalEncryptedAmount,
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

        /// `confidentialTransferFromAndCall` (ACL).
        #[pallet::call_index(9)]
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

        /// Disclose a ciphertext to a plaintext `u64` if backend allows (policy-specific).
        #[pallet::call_index(10)]
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

    // ------------------------------
    // Internal helpers
    // ------------------------------

    impl<T: Config> Pallet<T> {
        fn ensure_is_self_or_operator(
            holder: &T::AccountId,
            asset: &T::AssetId,
            caller: &T::AccountId,
        ) -> Result<(), Error<T>> {
            if caller == holder {
                return Ok(());
            }
            let now = <frame_system::Pallet<T>>::block_number();
            match Operators::<T>::get((holder, asset, caller)) {
                Some(until) if now <= until => Ok(()),
                Some(_) => Err(Error::<T>::OperatorExpired),
                None => Err(Error::<T>::NotAuthorized),
            }
        }
    }
}
