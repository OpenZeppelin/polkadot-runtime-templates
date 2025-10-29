// pallets/confidential-xcm-bridge/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::RuntimeDebug;
    use sp_std::prelude::*;

    use confidential_assets_primitives::{
        ConfidentialBackend, ConfidentialSwap, EncryptedAmount, InputProof, Ramp,
    };

    // Helper: associated SwapId type of the configured swap pallet
    pub type SwapIdOf<T> = <<T as Config>::Swap as ConfidentialSwap<
        <T as frame_system::Config>::AccountId,
        <T as Config>::AssetId,
        <T as Config>::Balance,
    >>::SwapId;

    /// Backend extension used when crediting an incoming encrypted delta on the destination chain.
    pub trait BridgeBackend<AccountId, AssetId, Balance>:
        ConfidentialBackend<AccountId, AssetId, Balance>
    {
        fn credit_incoming_encrypted(
            asset: AssetId,
            who: &AccountId,
            delta: EncryptedAmount,
            proof: InputProof,
        ) -> Result<EncryptedAmount, DispatchError>;
    }

    /// Very small abstraction so this pallet doesn’t hard-code any XCM version or pallet.
    /// Implement in the runtime by delegating to pallet-xcm or Moonbeam’s xcm-transactor.
    pub trait XcmRouter {
        type ParaId: Parameter + Copy + MaxEncodedLen + TypeInfo;
        type Weight: Parameter + Copy + MaxEncodedLen + TypeInfo + Default;
        type FeeAssetId: Parameter + Copy + MaxEncodedLen + TypeInfo;
        type FeeBalance: Parameter + Copy + MaxEncodedLen + TypeInfo;

        fn send_transact(
            dest: Self::ParaId,
            payload: Vec<u8>,
            fee_asset: Self::FeeAssetId,
            fee: Self::FeeBalance,
            weight_limit: Self::Weight,
        ) -> Result<(), DispatchError>;
    }

    // --------- SCALE payloads carried by XCM::Transact ---------

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
    pub enum RemoteCall<AccountId, AssetId, Balance, SwapId> {
        ReceiveConfidentialTransfer {
            sender_on_src: [u8; 32],
            dest_account: AccountId,
            asset: AssetId,
            delta_ciphertext: EncryptedAmount,
            proof: InputProof,
        },
        ExecuteConfidentialSwap(RemoteSwap<AccountId, AssetId, Balance, SwapId>),
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
    pub enum RemoteSwap<AccountId, AssetId, Balance, SwapId> {
        /// Accept an existing C↔C intent by id on the destination chain.
        ConfToConfById {
            who: AccountId,
            id: SwapId,
            // taker leg (counterparty -> proposer) to satisfy maker's terms
            b_to_a_ct: EncryptedAmount,
            b_to_a_proof: InputProof,
        },
        /// Accept an existing C→P intent by id on the destination chain.
        ConfToTransById { who: AccountId, id: SwapId },
        /// Transparent→Confidential single-shot (optional / may be unsupported).
        TransToConf {
            who: AccountId,
            give_asset: AssetId,
            want_asset: AssetId,
            give_amount: Balance,
            min_recv_hint: Option<u128>,
        },
    }

    // --------- Config ---------

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AssetId: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo;
        type Balance: Parameter + Member + Copy + Ord + MaxEncodedLen + TypeInfo + Default;

        /// Your backend must also support crediting incoming deltas.
        type Backend: BridgeBackend<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Existing on/off-ramp abstraction (not used by this pallet yet, but kept for symmetry).
        type Ramp: Ramp<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Router that actually composes/sends the XCM Transact.
        type Xcm: XcmRouter<
            ParaId = Self::ParaId,
            Weight = Self::XcmWeight,
            FeeAssetId = Self::FeeAssetId,
            FeeBalance = Self::FeeBalance,
        >;

        /// Bind your swaps pallet here (just like Backend/Ramp).
        type Swap: ConfidentialSwap<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Concrete types for the router.
        type ParaId: Parameter + Copy + MaxEncodedLen + TypeInfo;
        type XcmWeight: Parameter + Copy + MaxEncodedLen + TypeInfo + Default;
        type FeeAssetId: Parameter + Copy + MaxEncodedLen + TypeInfo;
        type FeeBalance: Parameter + Copy + MaxEncodedLen + TypeInfo;

        type WeightInfo: WeightInfo;
    }

    // --------- Weights ---------

    pub trait WeightInfo {
        fn send_confidential_transfer() -> Weight;
        fn send_confidential_swap() -> Weight;
        fn xcm_receive_confidential_transfer() -> Weight;
        fn xcm_execute_confidential_swap() -> Weight;
    }
    impl WeightInfo for () {
        fn send_confidential_transfer() -> Weight {
            Weight::from_parts(20_000, 0)
        }
        fn send_confidential_swap() -> Weight {
            Weight::from_parts(25_000, 0)
        }
        fn xcm_receive_confidential_transfer() -> Weight {
            Weight::from_parts(25_000, 0)
        }
        fn xcm_execute_confidential_swap() -> Weight {
            Weight::from_parts(30_000, 0)
        }
    }

    // --------- Storage ---------

    #[pallet::storage]
    #[pallet::getter(fn next_nonce)]
    pub type NextNonce<T> = StorageValue<_, u64, ValueQuery>;

    // --------- Events / Errors ---------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        XcmSent {
            nonce: u64,
            dest: T::ParaId,
            payload_hash: [u8; 32],
        },
        XcmConfTransferApplied {
            from_tag: [u8; 32],
            to: T::AccountId,
            asset: T::AssetId,
        },
        XcmConfSwapExecuted {
            who: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        RouterError,
        BackendError,
        SwapFailed,
        BadOriginForXcm, // replace with EnsureXcm in runtime
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // --------- Calls ---------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Source-chain: send a confidential transfer to destination chain.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::send_confidential_transfer())]
        pub fn send_confidential_transfer(
            origin: OriginFor<T>,
            dest: T::ParaId,
            sender_tag: [u8; 32],
            beneficiary: T::AccountId,
            asset: T::AssetId,
            delta_ciphertext: EncryptedAmount,
            proof: InputProof,
            fee_asset: T::FeeAssetId,
            fee: T::FeeBalance,
            weight_limit: T::XcmWeight,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let call = RemoteCall::<T::AccountId, T::AssetId, T::Balance, SwapIdOf<T>>::ReceiveConfidentialTransfer {
                sender_on_src: sender_tag,
                dest_account: beneficiary,
                asset,
                delta_ciphertext,
                proof,
            };
            let payload = call.encode();
            let payload_hash = sp_io::hashing::blake2_256(&payload);

            T::Xcm::send_transact(dest, payload, fee_asset, fee, weight_limit)
                .map_err(|_| Error::<T>::RouterError)?;

            let nonce = NextNonce::<T>::mutate(|n| {
                let cur = *n;
                *n = n.saturating_add(1);
                cur
            });

            Self::deposit_event(Event::XcmSent {
                nonce,
                dest,
                payload_hash,
            });
            Ok(())
        }

        /// Source-chain: request a remote confidential swap on destination chain.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::send_confidential_swap())]
        pub fn send_confidential_swap(
            origin: OriginFor<T>,
            dest: T::ParaId,
            payload: RemoteSwap<T::AccountId, T::AssetId, T::Balance, SwapIdOf<T>>,
            fee_asset: T::FeeAssetId,
            fee: T::FeeBalance,
            weight_limit: T::XcmWeight,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let call = RemoteCall::<T::AccountId, T::AssetId, T::Balance, SwapIdOf<T>>::ExecuteConfidentialSwap(payload);
            let encoded = call.encode();
            let payload_hash = sp_io::hashing::blake2_256(&encoded);

            T::Xcm::send_transact(dest, encoded, fee_asset, fee, weight_limit)
                .map_err(|_| Error::<T>::RouterError)?;

            let nonce = NextNonce::<T>::mutate(|n| {
                let cur = *n;
                *n = n.saturating_add(1);
                cur
            });

            Self::deposit_event(Event::XcmSent {
                nonce,
                dest,
                payload_hash,
            });
            Ok(())
        }

        // ---------------- Inbound handlers (gate with EnsureXcm in runtime) ----------------

        /// Destination-chain: credit the incoming confidential transfer.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::xcm_receive_confidential_transfer())]
        pub fn xcm_receive_confidential_transfer(
            origin: OriginFor<T>,
            sender_on_src: [u8; 32],
            dest_account: T::AccountId,
            asset: T::AssetId,
            delta_ciphertext: EncryptedAmount,
            proof: InputProof,
        ) -> DispatchResult {
            ensure_root(origin).map_err(|_| Error::<T>::BadOriginForXcm)?;

            T::Backend::credit_incoming_encrypted(asset, &dest_account, delta_ciphertext, proof)
                .map_err(|_| Error::<T>::BackendError)?;

            Self::deposit_event(Event::XcmConfTransferApplied {
                from_tag: sender_on_src,
                to: dest_account,
                asset,
            });
            Ok(())
        }

        /// Destination-chain: execute the requested confidential swap on this chain.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::xcm_execute_confidential_swap())]
        pub fn xcm_execute_confidential_swap(
            origin: OriginFor<T>,
            payload: RemoteSwap<T::AccountId, T::AssetId, T::Balance, SwapIdOf<T>>,
        ) -> DispatchResult {
            ensure_root(origin).map_err(|_| Error::<T>::BadOriginForXcm)?;

            // Capture `who` for event.
            let who_for_event: T::AccountId = match &payload {
                RemoteSwap::ConfToConfById { who, .. } => who.clone(),
                RemoteSwap::ConfToTransById { who, .. } => who.clone(),
                RemoteSwap::TransToConf { who, .. } => who.clone(),
            };

            match payload {
                RemoteSwap::ConfToConfById {
                    who,
                    id,
                    b_to_a_ct,
                    b_to_a_proof,
                } => {
                    T::Swap::swap_confidential_exact_in(&who, id, b_to_a_ct, b_to_a_proof)
                        .map_err(|_| Error::<T>::SwapFailed)?;
                }
                RemoteSwap::ConfToTransById { who, id } => {
                    T::Swap::swap_conf_to_transparent_exact_in(&who, id)
                        .map_err(|_| Error::<T>::SwapFailed)?;
                }
                RemoteSwap::TransToConf {
                    who,
                    give_asset,
                    want_asset,
                    give_amount,
                    min_recv_hint,
                } => {
                    T::Swap::swap_transparent_to_conf_exact_in(
                        &who,
                        give_asset,
                        want_asset,
                        give_amount,
                        min_recv_hint,
                    )
                    .map_err(|_| Error::<T>::SwapFailed)?;
                }
            }

            Self::deposit_event(Event::XcmConfSwapExecuted { who: who_for_event });
            Ok(())
        }
    }
}
