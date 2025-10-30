// pallets/confidential-htlc/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Get, PalletId};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, CheckedSub};
use sp_std::prelude::*;

/// --- Optional: plug in your confidential primitives when ready ---
/// use confidential_assets_primitives::{EncryptedAmount, InputProof, ConfidentialBackend};

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    // ---------------------------
    // Backend: escrow + crypto glue
    // ---------------------------

    /// Escrow/crypto backend for HTLC. Keep your cryptography and value movement here.
    /// Implement this via your confidential-assets pallet or a thin adapter pallet.
    pub trait Backend<AccountId, AssetId, Balance> {
        /// Hash type used for hashlocks (e.g., [u8; 32]).
        type HashLock: Parameter + MaxEncodedLen + TypeInfo + Copy + Eq;
        /// Secret type revealed on redeem (e.g., [u8; 32] or a scalar bytes).
        type Secret: Parameter + MaxEncodedLen + TypeInfo + Clone + Eq;

        /// Move value from `who` into pallet escrow. For confidential flows, you
        /// can encode `amount` however you like (e.g., 0 for opaque escrow) and
        /// verify proofs inside your impl.
        fn escrow_lock(
            asset: AssetId,
            who: &AccountId,
            amount: Balance,
            // For confidential mode you can accept extra proof bytes here if needed:
            extra: Option<Vec<u8>>,
        ) -> Result<(), DispatchError>;

        /// Release escrowed value to `to` (on successful redeem).
        fn escrow_release(
            asset: AssetId,
            to: &AccountId,
            amount: Balance,
            extra: Option<Vec<u8>>,
        ) -> Result<(), DispatchError>;

        /// Refund escrowed value to `to` (after timeout).
        fn escrow_refund(
            asset: AssetId,
            to: &AccountId,
            amount: Balance,
            extra: Option<Vec<u8>>,
        ) -> Result<(), DispatchError>;

        /// Compute H(secret) for hashlocks.
        fn hash_secret(s: &Self::Secret) -> Self::HashLock;

        /// Given (partial/adaptor_sig, final_sig) recover the embedded secret.
        /// If you don’t use adaptor-sigs, implement as `Err(BadSignature)`.
        fn recover_secret_from_sigs(
            partial: &[u8],
            final_sig: &[u8],
        ) -> Result<Self::Secret, DispatchError>;
    }

    /// Trait your BRIDGE pallet will depend on. This pallet implements it.
    pub trait BridgeHtlc<AccountId, AssetId, Balance> {
        type HashLock;
        type Secret;

        /// Create + fund an HTLC. Returns an identifier you can mirror cross-chain.
        fn open_htlc(
            maker: &AccountId,
            taker: Option<AccountId>,
            asset: AssetId,
            amount: Balance,
            hashlock: Self::HashLock,
            // absolute expiry block; refunds become valid at `>= expiry`
            expiry: u32,
            // Optional partial/adaptor signature commitment (for adaptor flow).
            adaptor_partial: Option<Vec<u8>>,
            // Optional backend-opaque bytes (proofs, commitments).
            backend_extra: Option<Vec<u8>>,
        ) -> Result<u64, DispatchError>;

        /// Redeem by preimage (classic HTLC). Returns the secret (for relaying).
        fn redeem_with_secret(
            who: &AccountId,
            htlc_id: u64,
            secret: Self::Secret,
            backend_extra: Option<Vec<u8>>,
        ) -> Result<Self::Secret, DispatchError>;

        /// Redeem by final signature; pallet recovers the secret and returns it.
        fn redeem_with_adaptor_sig(
            who: &AccountId,
            htlc_id: u64,
            final_sig: Vec<u8>,
            backend_extra: Option<Vec<u8>>,
        ) -> Result<Self::Secret, DispatchError>;

        /// Refund after expiry (maker only).
        fn refund(who: &AccountId, htlc_id: u64) -> DispatchResult;
    }

    // ---------------------------
    // Config
    // ---------------------------

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AssetId: Parameter + MaxEncodedLen + TypeInfo + Copy + Ord;
        type Balance: Parameter + MaxEncodedLen + TypeInfo + Copy + Ord + CheckedSub;

        /// Backend glue (escrow + crypto).
        type Backend: Backend<Self::AccountId, Self::AssetId, Self::Balance>;

        /// PalletId for the escrow account
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        type WeightInfo: WeightInfo;
    }

    pub trait WeightInfo {
        fn open_htlc() -> Weight;
        fn redeem_with_secret() -> Weight;
        fn redeem_with_adaptor_sig() -> Weight;
        fn refund() -> Weight;
    }
    impl WeightInfo for () {
        fn open_htlc() -> Weight {
            Weight::from_parts(20_000, 0)
        }
        fn redeem_with_secret() -> Weight {
            Weight::from_parts(25_000, 0)
        }
        fn redeem_with_adaptor_sig() -> Weight {
            Weight::from_parts(25_000, 0)
        }
        fn refund() -> Weight {
            Weight::from_parts(30_000, 0)
        }
    }

    // ---------------------------
    // Types & Storage
    // ---------------------------

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, RuntimeDebug)]
    pub enum HtlcState {
        Open,
        Redeemed,
        Refunded,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, RuntimeDebug)]
    pub struct Htlc<AccountId, AssetId, Balance, BlockNumber, HashLock> {
        pub maker: AccountId,
        pub taker: Option<AccountId>,
        pub asset: AssetId,
        pub amount: Balance,
        pub hashlock: HashLock,
        pub expiry: BlockNumber,
        pub adaptor_partial: Option<BoundedVec<u8, ConstU32<64>>>, // opaque (bytes for adaptor-sig commitment)
        pub backend_extra: BoundedVec<u8, ConstU32<64>>,           // tweak capacity as needed
        pub state: HtlcState,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Monotonic HTLC id counter.
    #[pallet::storage]
    pub(super) type NextId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// htlc_id -> record
    #[pallet::storage]
    pub(super) type Htlcs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        Htlc<
            T::AccountId,
            T::AssetId,
            T::Balance,
            BlockNumberFor<T>,
            <T::Backend as Backend<T::AccountId, T::AssetId, T::Balance>>::HashLock,
        >,
        OptionQuery,
    >;

    // ---------------------------
    // Events / Errors
    // ---------------------------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        HtlcOpened {
            id: u64,
            maker: T::AccountId,
            taker: Option<T::AccountId>,
            asset: T::AssetId,
            amount: T::Balance,
            expiry: BlockNumberFor<T>,
        },
        HtlcRedeemed {
            id: u64,
            redeemer: T::AccountId,
            secret: Vec<u8>,
        },
        HtlcRefunded {
            id: u64,
            maker: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        NotFound,
        NotOpen,
        NotAuthorized,
        NotYetExpired,
        BadSecret,
        BadSignature,
        Arithmetic,
    }

    // ---------------------------
    // Pallet account (optional)
    // ---------------------------

    impl<T: Config> Pallet<T> {
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }
    }

    // ---------------------------
    // Calls (extrinsics)
    // ---------------------------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Maker opens + funds an HTLC. Escrows `amount`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::open_htlc())]
        pub fn open_htlc(
            origin: OriginFor<T>,
            taker: Option<T::AccountId>,
            asset: T::AssetId,
            amount: T::Balance,
            hashlock: <T::Backend as Backend<T::AccountId, T::AssetId, T::Balance>>::HashLock,
            expiry: BlockNumberFor<T>,
            adaptor_partial: Option<Vec<u8>>,
            backend_extra: Option<Vec<u8>>,
        ) -> DispatchResult {
            let maker = ensure_signed(origin)?;

            // Lock into escrow
            T::Backend::escrow_lock(asset, &maker, amount, backend_extra.clone())
                .map_err(|_| Error::<T>::Arithmetic)?;

            // Store HTLC
            let id = NextId::<T>::mutate(|x| {
                let id = *x;
                *x = x.saturating_add(1);
                id
            });

            // Bound and store extras
            let backend_bounded: BoundedVec<_, ConstU32<64>> = backend_extra
                .clone()
                .unwrap_or_default()
                .try_into()
                .map_err(|_| Error::<T>::Arithmetic)?;

            let adaptor_bounded: Option<BoundedVec<_, ConstU32<64>>> = match adaptor_partial {
                Some(bytes) => Some(bytes.try_into().map_err(|_| Error::<T>::Arithmetic)?),
                None => None,
            };

            let rec = Htlc::<T::AccountId, T::AssetId, T::Balance, BlockNumberFor<T>, _> {
                maker: maker.clone(),
                taker,
                asset,
                amount,
                hashlock,
                expiry,
                adaptor_partial: adaptor_bounded,
                backend_extra: backend_bounded,
                state: HtlcState::Open,
            };
            Htlcs::<T>::insert(id, rec);

            let taker_for_event = Htlcs::<T>::get(id).and_then(|r| r.taker);
            Self::deposit_event(Event::HtlcOpened {
                id,
                maker,
                taker: taker_for_event,
                asset,
                amount,
                expiry,
            });
            Ok(())
        }

        /// Redeem with preimage `secret`. `who` must be the taker if specified, else anyone presenting the valid secret.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::redeem_with_secret())]
        pub fn redeem_with_secret(
            origin: OriginFor<T>,
            htlc_id: u64,
            secret: <T::Backend as Backend<T::AccountId, T::AssetId, T::Balance>>::Secret,
            backend_extra: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);
            if let Some(taker) = &rec.taker {
                ensure!(&who == taker, Error::<T>::NotAuthorized);
            }

            // Check hashlock
            let h = T::Backend::hash_secret(&secret);
            ensure!(h == rec.hashlock, Error::<T>::BadSecret);

            // Release escrow to taker (or to `who`)
            let to = rec.taker.as_ref().unwrap_or(&who);
            T::Backend::escrow_release(rec.asset, to, rec.amount, backend_extra)
                .map_err(|_| Error::<T>::Arithmetic)?;

            rec.state = HtlcState::Redeemed;
            Htlcs::<T>::insert(htlc_id, &rec);

            // Emit secret bytes so the *other* chain can learn it (bridge watches this)
            let secret_bytes = secret.encode();
            Self::deposit_event(Event::HtlcRedeemed {
                id: htlc_id,
                redeemer: who,
                secret: secret_bytes,
            });
            Ok(())
        }

        /// Redeem with final signature. Pallet recovers the secret using (partial, final),
        /// verifies hashlock, releases escrow, and emits the recovered secret.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::redeem_with_adaptor_sig())]
        pub fn redeem_with_adaptor_sig(
            origin: OriginFor<T>,
            htlc_id: u64,
            final_sig: Vec<u8>,
            backend_extra: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);
            if let Some(taker) = &rec.taker {
                ensure!(&who == taker, Error::<T>::NotAuthorized);
            }
            let partial = rec
                .adaptor_partial
                .clone()
                .ok_or(Error::<T>::BadSignature)?;

            // Recover secret
            let secret =
                T::Backend::recover_secret_from_sigs(partial.as_slice(), final_sig.as_slice())
                    .map_err(|_| Error::<T>::BadSignature)?;

            // Check hashlock
            let h = T::Backend::hash_secret(&secret);
            ensure!(h == rec.hashlock, Error::<T>::BadSecret);

            // Release escrow to taker (or `who`)
            let to = rec.taker.as_ref().unwrap_or(&who);
            T::Backend::escrow_release(rec.asset, to, rec.amount, backend_extra)
                .map_err(|_| Error::<T>::Arithmetic)?;

            rec.state = HtlcState::Redeemed;
            Htlcs::<T>::insert(htlc_id, &rec);

            let secret_bytes = secret.encode();
            Self::deposit_event(Event::HtlcRedeemed {
                id: htlc_id,
                redeemer: who,
                secret: secret_bytes,
            });
            Ok(())
        }

        /// Refund to maker after expiry.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::refund())]
        pub fn refund(
            origin: OriginFor<T>,
            htlc_id: u64,
            backend_extra: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);
            ensure!(who == rec.maker, Error::<T>::NotAuthorized);
            ensure!(
                frame_system::Pallet::<T>::block_number() >= rec.expiry,
                Error::<T>::NotYetExpired
            );

            T::Backend::escrow_refund(rec.asset, &rec.maker, rec.amount, backend_extra)
                .map_err(|_| Error::<T>::Arithmetic)?;

            rec.state = HtlcState::Refunded;
            Htlcs::<T>::insert(htlc_id, &rec);

            Self::deposit_event(Event::HtlcRefunded {
                id: htlc_id,
                maker: who,
            });
            Ok(())
        }
    }

    // ---------------------------
    // BridgeHtlc impl (for your bridge pallet to call internally if you prefer)
    // ---------------------------

    impl<T: Config> BridgeHtlc<T::AccountId, T::AssetId, T::Balance> for Pallet<T> {
        type HashLock = <T::Backend as Backend<T::AccountId, T::AssetId, T::Balance>>::HashLock;
        type Secret = <T::Backend as Backend<T::AccountId, T::AssetId, T::Balance>>::Secret;

        fn open_htlc(
            maker: &T::AccountId,
            taker: Option<T::AccountId>,
            asset: T::AssetId,
            amount: T::Balance,
            hashlock: Self::HashLock,
            expiry_abs: u32,
            adaptor_partial: Option<Vec<u8>>,
            backend_extra: Option<Vec<u8>>,
        ) -> Result<u64, DispatchError> {
            // Just call the extrinsic-side logic sans origin; mirror essential steps.
            T::Backend::escrow_lock(asset, maker, amount, backend_extra.clone())?;

            let id = NextId::<T>::mutate(|x| {
                let id = *x;
                *x = x.saturating_add(1);
                id
            });
            let expiry_bn: BlockNumberFor<T> = expiry_abs.into();

            let backend_bounded: BoundedVec<_, ConstU32<64>> = backend_extra
                .clone()
                .unwrap_or_default()
                .try_into()
                .map_err(|_| Error::<T>::Arithmetic)?;

            let adaptor_bounded: Option<BoundedVec<_, ConstU32<64>>> = match adaptor_partial {
                Some(bytes) => Some(bytes.try_into().map_err(|_| Error::<T>::Arithmetic)?),
                None => None,
            };

            let rec = Htlc::<T::AccountId, T::AssetId, T::Balance, BlockNumberFor<T>, _> {
                maker: maker.clone(),
                taker,
                asset,
                amount,
                hashlock,
                expiry: expiry_bn,
                adaptor_partial: adaptor_bounded,
                backend_extra: backend_bounded,
                state: HtlcState::Open,
            };
            Htlcs::<T>::insert(id, rec);
            Ok(id)
        }

        fn redeem_with_secret(
            who: &T::AccountId,
            htlc_id: u64,
            secret: Self::Secret,
            backend_extra: Option<Vec<u8>>,
        ) -> Result<Self::Secret, DispatchError> {
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            if let Some(taker) = &rec.taker {
                ensure!(who == taker, Error::<T>::NotAuthorized);
            }
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);
            ensure!(
                T::Backend::hash_secret(&secret) == rec.hashlock,
                Error::<T>::BadSecret
            );

            let to = rec.taker.as_ref().unwrap_or(who);
            T::Backend::escrow_release(rec.asset, to, rec.amount, backend_extra)?;
            rec.state = HtlcState::Redeemed;
            Htlcs::<T>::insert(htlc_id, &rec);
            Ok(secret)
        }

        fn redeem_with_adaptor_sig(
            who: &T::AccountId,
            htlc_id: u64,
            final_sig: Vec<u8>,
            backend_extra: Option<Vec<u8>>,
        ) -> Result<Self::Secret, DispatchError> {
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            if let Some(taker) = &rec.taker {
                ensure!(who == taker, Error::<T>::NotAuthorized);
            }
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);

            let partial = rec
                .adaptor_partial
                .clone()
                .ok_or(Error::<T>::BadSignature)?;
            let secret =
                T::Backend::recover_secret_from_sigs(partial.as_slice(), final_sig.as_slice())?;
            ensure!(
                T::Backend::hash_secret(&secret) == rec.hashlock,
                Error::<T>::BadSecret
            );

            let to = rec.taker.as_ref().unwrap_or(who);
            T::Backend::escrow_release(rec.asset, to, rec.amount, backend_extra)?;
            rec.state = HtlcState::Redeemed;
            Htlcs::<T>::insert(htlc_id, &rec);
            Ok(secret)
        }

        fn refund(who: &T::AccountId, htlc_id: u64) -> DispatchResult {
            let mut rec = Htlcs::<T>::get(htlc_id).ok_or(Error::<T>::NotFound)?;
            ensure!(matches!(rec.state, HtlcState::Open), Error::<T>::NotOpen);
            ensure!(who == &rec.maker, Error::<T>::NotAuthorized);
            ensure!(
                frame_system::Pallet::<T>::block_number() >= rec.expiry,
                Error::<T>::NotYetExpired
            );
            T::Backend::escrow_refund(rec.asset, &rec.maker, rec.amount, None)?;
            rec.state = HtlcState::Refunded;
            Htlcs::<T>::insert(htlc_id, &rec);
            Ok(())
        }
    }
}
