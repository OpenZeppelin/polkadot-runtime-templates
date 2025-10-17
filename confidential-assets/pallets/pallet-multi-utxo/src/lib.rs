#![cfg_attr(not(feature = "std"), no_std)]

//! # pallet-multi-utxo
//!
//! Minimal, **multi-asset** UTXO storage & transfer core intended as a
//! foundation for higher-level privacy layers (e.g., MimbleWimble cut-through).
//!
//! ## Design goals
//! - **Isolation:** keep UTXO core free of MW/ZK specifics so other layers can plug in.
//! - **Multi-asset:** storage is keyed by `AssetId` first to shard naturally.
//! - **KISS & Efficient Enough:** use a `DoubleMap<(AssetId, UtxoId)> -> Utxo`.
//! - **Deterministic IDs:** `UtxoId = blake2_256(asset_id || encoded(utxo) || salt)`.
//! - **Conservation per Asset:** `spend` enforces sum(inputs) == sum(outputs) per asset.
//!
//! ## What this pallet does
//! - `mint`: Privileged origin (e.g., Root or Council) can mint new UTXOs.
//! - `spend`: Signed origin consumes owned inputs and creates outputs, balanced per asset.
//!
//! ## What it intentionally doesn't do (yet)
//! - No signatures over arbitrary keys (uses `AccountId` ownership for simplicity).
//! - No MW-specific range proofs, no cut-through, no confidentiality.
//! - No fees or dust rules (add later in MW layer).
//!
//! ## Extending
//! - Wrap/replace `owner: AccountId` with opaque keys later (e.g., MW pubkeys).
//! - Add `Verifier` trait (ZK/MW) and call hooks inside `spend` before state changes.
//! - Consider child tries per AssetId if need isolated proofs. KISS for now.

use frame_support::{ensure, pallet_prelude::*, traits::Get, BoundedVec};
use frame_system::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash, Saturating, Zero};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Compact newtype for a UTXO identifier.
    pub type UtxoId = H256;

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, RuntimeDebug)]
    pub struct Utxo<AccountId, Balance> {
        /// Current owner. For MW, replace with opaque key commitment later.
        pub owner: AccountId,
        /// Amount for this output (per-asset). Units are arbitrary; MW will swap to commitments.
        pub value: Balance,
    }

    /// Output descriptor including a salt for deterministic id generation.
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, RuntimeDebug)]
    pub struct NewOutput<AccountId, Balance, AssetId> {
        pub asset_id: AssetId,
        pub utxo: Utxo<AccountId, Balance>,
        /// Salt to make `UtxoId` unique when `(asset, utxo)` collides (e.g., same owner/value).
        pub salt: u64,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Asset identifier type (e.g., `u32` or `u128`).
        type AssetId: Parameter + Member + Ord + Copy + Default + MaxEncodedLen + TypeInfo;

        /// Balance type for UTXO values.
        type Balance: Parameter
            + Member
            + Copy
            + Default
            + MaxEncodedLen
            + TypeInfo
            + Saturating
            + Zero
            + Ord;

        /// Max number of inputs per `spend`.
        #[pallet::constant]
        type MaxInputs: Get<u32>;

        /// Max number of outputs per `mint`/`spend`.
        #[pallet::constant]
        type MaxOutputs: Get<u32>;

        /// Origin allowed to `mint` UTXOs.
        type MintOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Weight info (minimal default provided).
        type WeightInfo: WeightInfo;
    }

    /// Double-map UTXO set: shard by AssetId first for better locality.
    #[pallet::storage]
    #[pallet::getter(fn utxos)]
    pub type Utxos<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        UtxoId,
        Utxo<T::AccountId, T::Balance>,
        OptionQuery,
    >;

    /// (Optional) simple supply tracker per asset to sanity-check conservation across extrinsics.
    #[pallet::storage]
    #[pallet::getter(fn total_issued)]
    pub type TotalIssued<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AssetId, T::Balance, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// UTXO created: (asset_id, utxo_id, owner, value)
        UtxoCreated(T::AssetId, UtxoId, T::AccountId, T::Balance),
        /// UTXO spent: (asset_id, utxo_id, owner)
        UtxoSpent(T::AssetId, UtxoId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Input/Output vector length exceeds configured bounds.
        TooManyItems,
        /// Provided input not found in UTXO set.
        UnknownInput,
        /// Attempted to spend a UTXO not owned by the signer.
        NotOwner,
        /// Conservation rule violated for at least one asset.
        UnbalancedPerAsset,
        /// Output UTXO already exists (salt collision).
        OutputAlreadyExists,
        /// Zero value outputs are not allowed (KISS rule; adjust if needed).
        ZeroValue,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Privileged mint: create new UTXOs without inputs (e.g., bridging, faucets, genesis issuance).
        ///
        /// - Fails if `outputs.len() > MaxOutputs`.
        /// - Updates `TotalIssued[asset]` by sum of values.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::mint(outputs.len() as u32))]
        pub fn mint(
            origin: OriginFor<T>,
            outputs: Vec<NewOutput<T::AccountId, T::Balance, T::AssetId>>,
        ) -> DispatchResult {
            T::MintOrigin::ensure_origin(origin)?;
            let bounded: BoundedVec<_, T::MaxOutputs> = outputs
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::TooManyItems)?;

            // Insert all outputs; compute ids deterministically.
            for o in bounded.into_inner() {
                ensure!(!o.utxo.value.is_zero(), Error::<T>::ZeroValue);
                let id = Self::calc_utxo_id(&o.asset_id, &o.utxo, o.salt);
                ensure!(
                    !Utxos::<T>::contains_key(&o.asset_id, &id),
                    Error::<T>::OutputAlreadyExists
                );
                Utxos::<T>::insert(&o.asset_id, id, &o.utxo);
                // accounting
                TotalIssued::<T>::mutate(o.asset_id, |b| *b = b.saturating_add(o.utxo.value));
                Self::deposit_event(Event::UtxoCreated(
                    o.asset_id,
                    id,
                    o.utxo.owner.clone(),
                    o.utxo.value,
                ));
            }
            Ok(())
        }

        /// Spend inputs owned by caller and create new outputs, balanced **per asset**.
        ///
        /// - Checks ownership for each input.
        /// - Enforces: sum(inputs[asset]) == sum(outputs[asset]) for all touched assets.
        /// - Deletes inputs, inserts outputs atomically.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::spend(inputs.len() as u32, outputs.len() as u32))]
        pub fn spend(
            origin: OriginFor<T>,
            inputs: Vec<(T::AssetId, UtxoId)>,
            outputs: Vec<NewOutput<T::AccountId, T::Balance, T::AssetId>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let in_bounded: BoundedVec<_, T::MaxInputs> = inputs
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::TooManyItems)?;
            let out_bounded: BoundedVec<_, T::MaxOutputs> = outputs
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::TooManyItems)?;

            // Tally by asset.
            let mut in_sums: BTreeMap<T::AssetId, T::Balance> = BTreeMap::new();
            let mut out_sums: BTreeMap<T::AssetId, T::Balance> = BTreeMap::new();

            // Validate inputs exist & ownership; accumulate sums.
            for (asset, id) in in_bounded.iter() {
                let utxo = Utxos::<T>::get(asset, id).ok_or(Error::<T>::UnknownInput)?;
                ensure!(utxo.owner == who, Error::<T>::NotOwner);
                in_sums
                    .entry(*asset)
                    .and_modify(|b| *b = b.saturating_add(utxo.value))
                    .or_insert(utxo.value);
            }

            // Prepare outputs; compute ids; check collisions & zero values; accumulate sums.
            let mut new_ids: Vec<(T::AssetId, UtxoId, Utxo<T::AccountId, T::Balance>)> =
                Vec::with_capacity(out_bounded.len());
            for o in out_bounded.iter() {
                ensure!(!o.utxo.value.is_zero(), Error::<T>::ZeroValue);
                let id = Self::calc_utxo_id(&o.asset_id, &o.utxo, o.salt);
                ensure!(
                    !Utxos::<T>::contains_key(&o.asset_id, &id),
                    Error::<T>::OutputAlreadyExists
                );
                out_sums
                    .entry(o.asset_id)
                    .and_modify(|b| *b = b.saturating_add(o.utxo.value))
                    .or_insert(o.utxo.value);
                new_ids.push((o.asset_id, id, o.utxo.clone()));
            }

            // Enforce conservation per asset: every asset touched by inputs or outputs must balance.
            // (Allows asset sets to differ only if both sides are zero, which is fine.)
            for asset in in_sums.keys().chain(out_sums.keys()) {
                let in_sum = *in_sums.get(asset).unwrap_or(&T::Balance::zero());
                let out_sum = *out_sums.get(asset).unwrap_or(&T::Balance::zero());
                ensure!(in_sum == out_sum, Error::<T>::UnbalancedPerAsset);
            }

            // State transition (atomic).
            // Remove inputs.
            for (asset, id) in in_bounded.into_inner() {
                let spent = Utxos::<T>::take(asset, id).ok_or(Error::<T>::UnknownInput)?;
                Self::deposit_event(Event::UtxoSpent(asset, id, spent.owner));
            }
            // Insert outputs.
            for (asset, id, utxo) in new_ids.into_iter() {
                Utxos::<T>::insert(asset, id, &utxo);
                Self::deposit_event(Event::UtxoCreated(asset, id, utxo.owner, utxo.value));
            }

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Deterministically derive a `UtxoId`.
        #[inline]
        pub fn calc_utxo_id(
            asset: &T::AssetId,
            utxo: &Utxo<T::AccountId, T::Balance>,
            salt: u64,
        ) -> UtxoId {
            // Hash: H(asset_id || utxo || salt)
            let mut enc = asset.using_encoded(|b| b.to_vec());
            utxo.using_encoded(|b| enc.extend_from_slice(b));
            salt.using_encoded(|b| enc.extend_from_slice(b));
            BlakeTwo256::hash(&enc)
        }
    }

    // -------- Weights (KISS) --------

    /// Minimal weight trait; plug in benchmarked values later.
    pub trait WeightInfo {
        fn mint(outputs: u32) -> Weight;
        fn spend(inputs: u32, outputs: u32) -> Weight;
    }

    impl WeightInfo for () {
        fn mint(outputs: u32) -> Weight {
            // Very rough linear model; replace with pallet benchmarks later.
            // db writes ~1 per output + one supply map write.
            let base = 10_000;
            let per = 25_000;
            Weight::from_parts(base as u64 + (per as u64) * outputs as u64, 0)
        }
        fn spend(inputs: u32, outputs: u32) -> Weight {
            // db reads (inputs) + existence checks (outputs) + writes (inputs + outputs)
            let base = 15_000;
            let per_in = 30_000;
            let per_out = 30_000;
            Weight::from_parts(
                base as u64 + (per_in as u64) * inputs as u64 + (per_out as u64) * outputs as u64,
                0,
            )
        }
    }
}
