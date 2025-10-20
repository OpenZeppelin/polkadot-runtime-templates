#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-confidential-assets (Multi-Asset, MW-backed, IERC-7984-inspired)
//!
//! User-facing pallet that exposes a **confidential fungible token surface** per `AssetId`,
//! delegating verification, state updates, balances, and totals to an abstract **MW/FHE backend**.
//!
//! ## Mirrors of IERC-7984 (adapted to Substrate + multi-asset)
//! - `setOperator(holder->operator, until)` → per (holder, asset, operator) with block expiry
//! - `confidentialTransfer(to, encryptedAmount, inputProof)` → `confidential_transfer_encrypted`
//! - `confidentialTransfer(to, amount)` (no proof; ACL-gated) → `confidential_transfer_acl`
//! - `confidentialTransferFrom(from, to, encryptedAmount, inputProof)`
//! - `confidentialTransferFrom(from, to, amount)` (no proof; ACL-gated)
//! - `*_and_call` variants call a runtime hook `OnConfidentialTransfer` with `(from, to, asset, transferred, data)`
//! - Read helpers: `confidential_total_supply`, `confidential_balance_of`, `is_operator`
//! - Metadata: `name/symbol/decimals/contract_uri` via `AssetMetadata` provider
//!
//! ## Layers
//! - `pallet-multi-utxo` → stores opaque commitments by asset.
//! - `pallet-mimblewimble` → verifies MW txs and writes commitments via UTXO backend.
//! - `pallet-confidential-assets` → operators, transfer surface, metadata proxy, receiver hooks.
//!
//! ## Opaque types & ACL
//! - Encrypted amount types are opaque bytes (to keep no_std & FHE-lib-agnostic).
//! - "ACL transfers" (no proof) must be authorized by the backend (e.g., view/ACL lists).
//!
//! ## You implement (in your backend pallets):
//! - `ConfidentialBackend` (totals, balances, transfer, ACL-transfer, disclose, optional tx submit)
//! - `AssetMetadataProvider` (name/symbol/decimals/contract_uri per AssetId)

use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

// ------------------------------
// Common opaque types (KISS)
// ------------------------------

/// Opaque ciphertext representing a "confidential" amount (analogous to `euint64`).
/// Keep bounded to protect PoV. Tune the size cap once your FHE/MW ciphertext is known.
pub type MaxCiphertextLen = ConstU32<128>;
pub type EncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

/// Opaque "external" encrypted amount (analogous to `externalEuint64`),
/// kept identical here (you may differentiate if your backend needs it).
pub type ExternalEncryptedAmount = BoundedVec<u8, MaxCiphertextLen>;

/// Proof/aux data blob used by the backend to validate "encrypted" transfers.
pub type MaxProofLen = ConstU32<8192>;
pub type InputProof = BoundedVec<u8, MaxProofLen>;

/// Optional data payload for `*_and_call` variants.
pub type MaxCallbackDataLen = ConstU32<4096>;
pub type CallbackData = BoundedVec<u8, MaxCallbackDataLen>;

// ------------------------------
// MW "raw tx" path (optional)
// ------------------------------

pub type Commitment = [u8; 32];

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct MwTransaction<AssetId> {
    pub asset_id: AssetId,
    pub input_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub output_commitments: BoundedVec<Commitment, ConstU32<64>>,
    pub excess_commitment: Commitment,
}

/// Optional raw MimbleWimble verification path in case you still submit whole MW txs.
pub trait MimbleWimbleCompute<AssetId> {
    fn verify_and_apply(tx: MwTransaction<AssetId>) -> DispatchResult;
}

// ------------------------------
// Backend abstraction
// ------------------------------

/// Backend that holds the **truth** for totals, balances, and executes transfers.
/// Implement this in your MW/FHE/UTXO layer (e.g., `pallet-mimblewimble` adapter).
pub trait ConfidentialBackend<AccountId, AssetId> {
    /// Returns the confidential total supply ciphertext for `asset`.
    fn total_supply(asset: AssetId) -> EncryptedAmount;

    /// Returns the confidential balance ciphertext for `who` under `asset`.
    /// Implementation may require a registered view key or ACL in the backend.
    fn balance_of(asset: AssetId, who: &AccountId) -> EncryptedAmount;

    /// Transfer using an **encrypted input** and a proof.
    /// Implement must debit `from` (determined by proof/ACL inside backend) and credit `to`,
    /// and return the encrypted amount actually transferred (may differ from input envelope).
    fn transfer_encrypted(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        encrypted_amount: ExternalEncryptedAmount,
        input_proof: InputProof,
    ) -> Result<EncryptedAmount, DispatchError>;

    /// Transfer using an **ACL-authorized** encrypted amount (no input proof given here).
    /// Backend must enforce that `from` (or operator) is already authorized for `amount`.
    fn transfer_acl(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        amount: EncryptedAmount,
    ) -> Result<EncryptedAmount, DispatchError>;

    /// Optional: disclose a ciphertext to a plaintext amount if backend policy allows.
    fn disclose_amount(
        asset: AssetId,
        encrypted_amount: &EncryptedAmount,
        who: &AccountId,
    ) -> Result<u64, DispatchError>;
}

/// Metadata provider per asset (names, symbols, etc.).
/// Implement against your multi-asset registry or a separate pallet.
pub trait AssetMetadataProvider<AssetId> {
    fn name(asset: AssetId) -> Vec<u8>;
    fn symbol(asset: AssetId) -> Vec<u8>;
    fn decimals(asset: AssetId) -> u8;
    fn contract_uri(asset: AssetId) -> Vec<u8>;
}

/// Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived).
/// Default `()` does nothing.
pub trait OnConfidentialTransfer<AccountId, AssetId> {
    /// Called after successful transfer if the extrinsic is an `*_and_call` variant.
    /// Return `Ok(())` to accept; `Err` to revert (pallet will bubble up error).
    fn on_confidential_transfer_received(
        from: &AccountId,
        to: &AccountId,
        asset: &AssetId,
        transferred: &EncryptedAmount,
        data: &CallbackData,
    ) -> Result<(), DispatchError>;
}

impl<AccountId, AssetId> OnConfidentialTransfer<AccountId, AssetId> for () {
    fn on_confidential_transfer_received(
        _from: &AccountId,
        _to: &AccountId,
        _asset: &AssetId,
        _transferred: &EncryptedAmount,
        _data: &CallbackData,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type used across the stack.
        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;

        /// Backend that verifies & applies confidential ops (MW/FHE/UTXO).
        type Backend: ConfidentialBackend<Self::AccountId, Self::AssetId>;

        /// Optional Raw MW path (keep your existing integration)
        type MwBackend: MimbleWimbleCompute<Self::AssetId>;

        /// Asset metadata provider (name/symbol/decimals/contract_uri).
        type AssetMetadata: AssetMetadataProvider<Self::AssetId>;

        /// Post-transfer receiver callback (no-op by default).
        type OnConfidentialTransfer: OnConfidentialTransfer<Self::AccountId, Self::AssetId>;

        /// Weight info
        type WeightInfo: WeightData;
    }

    /// Minimal weight trait; replace with real benches later.
    pub trait WeightData {
        fn set_operator() -> Weight;
        fn submit_confidential_tx(inputs: u32, outputs: u32) -> Weight;
        fn confidential_transfer() -> Weight;
        fn confidential_transfer_from() -> Weight;
        fn confidential_transfer_and_call() -> Weight;
        fn confidential_transfer_from_and_call() -> Weight;
        fn disclose_amount() -> Weight;
    }
    impl WeightData for () {
        fn set_operator() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn submit_confidential_tx(_i: u32, _o: u32) -> Weight {
            Weight::from_parts(20_000, 0)
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
        /// Raw MW path accepted a tx.
        ConfidentialTxApplied { asset: T::AssetId },
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

        // ---- RENAMED to avoid conflict with FRAME's Pallet::name() ----
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
    // Calls
    // ------------------------------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set/extend an operator for a holder & asset (no checks on `until` at write time).
        #[pallet::call_index(0)]
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

        /// Submit a raw MimbleWimble tx to the backend (optional path).
        #[pallet::call_index(1)]
        #[pallet::weight({
            let ins = tx.input_commitments.len() as u32;
            let outs = tx.output_commitments.len() as u32;
            T::WeightInfo::submit_confidential_tx(ins, outs)
        })]
        pub fn submit_confidential_tx(
            origin: OriginFor<T>,
            tx: MwTransaction<T::AssetId>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            <T as Config>::MwBackend::verify_and_apply(tx.clone())?;
            Self::deposit_event(Event::ConfidentialTxApplied { asset: tx.asset_id });
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
