#![cfg_attr(not(feature = "std"), no_std)]

// extern crate alloc;

pub mod ops;
// pub mod precompiles;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
pub use ops::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Operations exposed by the FHE coprocessor injected by the Runtime
        type RuntimeFhe: FheOps; //+ FhEVM;

        /// Weight info
        type WeightInfo: WeightData;
    }

    pub trait WeightData {
        fn set_operator() -> Weight;
        fn confidential_transfer() -> Weight;
        fn request_decryption() -> Weight;
        fn decrypt() -> Weight;
    }
    impl WeightData for () {
        fn set_operator() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn confidential_transfer() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn request_decryption() -> Weight {
            Weight::from_parts(10_000, 0)
        }

        fn decrypt() -> Weight {
            Weight::from_parts(10_000, 0)
        }
    }

    /// Storage for operators
    /// (Holder, Operator, AssetId) => Until
    #[pallet::storage]
    pub type Operators<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>, // Holder
            NMapKey<Blake2_128Concat, AssetId>,      // AssetId
            NMapKey<Blake2_128Concat, T::AccountId>, // Operator
        ),
        BlockNumberFor<T>, //Until
        OptionQuery,
    >;

    /// Storage for encrypted total supply
    /// AssetId => Cipher
    #[pallet::storage]
    pub type TotalSupply<T: Config> = StorageMap<_, Blake2_128Concat, AssetId, Cipher, OptionQuery>;

    /// Storage for encrypted balances
    /// (AssetId, Holder) => Cipher
    #[pallet::storage]
    pub type Balances<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AssetId,
        Blake2_128Concat,
        T::AccountId,
        Cipher,
        OptionQuery,
    >;

    #[pallet::storage]
    pub type DisclosureId<T: Config> = StorageValue<_, RequestId, ValueQuery>;

    /// Storage for requests to decrypt and publicly disclose encrypted amounts
    /// RequestId => Encrypted
    /// Decrypted amount emitted via Event and NOT stored here
    #[pallet::storage]
    pub type Disclosures<T: Config> =
        StorageMap<_, Blake2_128Concat, RequestId, Cipher, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        OperatorSet {
            asset: AssetId,
            holder: T::AccountId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        },
        ConfidentialTransfer {
            asset: AssetId,
            from: T::AccountId,
            to: T::AccountId,
            transferred: Cipher,
        },
        ConfidentialMint {
            asset: AssetId,
            to: T::AccountId,
            minted: Cipher,
        },
        ConfidentialBurn {
            asset: AssetId,
            from: T::AccountId,
            burned: Cipher,
        },
        DecryptionRequested {
            encrypted: Cipher,
        },
        AmountDisclosed {
            encrypted: Cipher,
            amount: Cipher,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        ZeroBalance,
        DisclosureIdOverflowed,
        DecryptionRequestNotFound,
        InvalidDecryptionProof,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_operator())]
        pub fn set_operator(
            origin: OriginFor<T>,
            asset: AssetId,
            operator: T::AccountId,
            until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            // Per ERC7984.sol do NOT check until < current_block (saves gas/weight)
            Operators::<T>::insert((holder.clone(), asset, operator.clone()), until);
            Self::deposit_event(Event::OperatorSet {
                asset,
                holder,
                operator,
                until,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_transfer(
            origin: OriginFor<T>,
            asset: AssetId,
            to: T::AccountId,
            amount: Cipher,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            let transferred =
                Self::try_confidential_transfer(&asset, Some(&from), Some(&to), &amount)?;
            Self::deposit_event(Event::ConfidentialTransfer {
                asset,
                from,
                to,
                transferred,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_mint(
            origin: OriginFor<T>,
            asset: AssetId,
            to: T::AccountId,
            amount: Cipher,
        ) -> DispatchResult {
            ensure_root(origin)?; //TODO configurable origin
            let minted = Self::try_confidential_transfer(&asset, None, Some(&to), &amount)?;
            Self::deposit_event(Event::ConfidentialMint { asset, to, minted });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::confidential_transfer())]
        pub fn confidential_burn(
            origin: OriginFor<T>,
            asset: AssetId,
            from: T::AccountId,
            amount: Cipher,
        ) -> DispatchResult {
            ensure_root(origin)?; //TODO configurable origin
            let burned = Self::try_confidential_transfer(&asset, Some(&from), None, &amount)?;
            Self::deposit_event(Event::ConfidentialBurn {
                asset,
                from,
                burned,
            });
            Ok(())
        }

        // Add back once FhEVM issue resolved
        // #[pallet::call_index(4)]
        // #[pallet::weight(T::WeightInfo::request_decryption())]
        // pub fn request_decryption(origin: OriginFor<T>, encrypted: Cipher) -> DispatchResult {
        //     ensure_signed(origin)?;
        //     let request = DisclosureId::<T>::get();
        //     Disclosures::<T>::insert(request, encrypted.clone());
        //     DisclosureId::<T>::set(
        //         request
        //             .checked_add(1)
        //             .ok_or(Error::<T>::DisclosureIdOverflowed)?,
        //     );
        //     <T as Config>::RuntimeFhe::request_decryption(encrypted.clone());
        //     Self::deposit_event(Event::DecryptionRequested { encrypted });
        //     Ok(())
        // }

        // #[pallet::call_index(5)]
        // #[pallet::weight(T::WeightInfo::decrypt())]
        // pub fn decrypt(
        //     origin: OriginFor<T>,
        //     request: RequestId,
        //     amount: Cipher, //plaintext
        //     proof: Cipher, //decryption proof (create new type?)
        // ) -> DispatchResult {
        //     let _ = ensure_signed(origin)?; //TODO: permissions?
        //     let encrypted =
        //         Disclosures::<T>::get(request).ok_or(Error::<T>::DecryptionRequestNotFound)?;
        //     ensure!(
        //         <T as Config>::RuntimeFhe::check_signatures(request, amount.clone(), proof),
        //         Error::<T>::InvalidDecryptionProof
        //     );
        //     Self::deposit_event(Event::AmountDisclosed { encrypted, amount });
        //     Ok(())
        // }
    }

    impl<T: Config> Pallet<T> {
        /// Try to execute confidential transfer
        /// Propagates error if unsuccessful otherwise returns amount transferred
        /// which is encrypted and hidden for all except participants
        fn try_confidential_transfer(
            asset: &AssetId,
            from: Option<&T::AccountId>,
            to: Option<&T::AccountId>,
            amount: &Cipher,
        ) -> Result<Cipher, Error<T>> {
            // debit or mint
            let (success, _) = if let Some(f) = from {
                let from_bal = Balances::<T>::get(asset.clone(), f.clone()).unwrap_or_default();
                ensure!(
                    <T as Config>::RuntimeFhe::is_initialized(from_bal.clone()),
                    Error::<T>::ZeroBalance
                );

                let (s, ptr) = <T as Config>::RuntimeFhe::try_decrease(from_bal, amount.clone());
                <T as Config>::RuntimeFhe::allow_this(ptr.clone());
                <T as Config>::RuntimeFhe::allow_to::<T>(ptr.clone(), f);

                Balances::<T>::insert(asset, f.clone(), ptr.clone());
                (s, ptr)
            } else {
                let total = TotalSupply::<T>::get(asset).unwrap_or_default();
                let (s, ptr) = <T as Config>::RuntimeFhe::try_increase(total, amount.clone());
                <T as Config>::RuntimeFhe::allow_this(ptr.clone());
                TotalSupply::<T>::insert(asset, ptr.clone());
                (s, ptr)
            };

            // transferred = success ? amount : 0
            let zero = <T as Config>::RuntimeFhe::as_zero();
            let transferred =
                <T as Config>::RuntimeFhe::select(success.clone(), amount.clone(), zero);

            // credit or burn
            if let Some(tacct) = to {
                let to_bal = Balances::<T>::get(asset.clone(), tacct.clone()).unwrap_or_default();
                let sum = <T as Config>::RuntimeFhe::add(to_bal, transferred.clone());
                <T as Config>::RuntimeFhe::allow_this(sum.clone());
                <T as Config>::RuntimeFhe::allow_to::<T>(sum.clone(), tacct);

                Balances::<T>::insert(asset.clone(), tacct.clone(), sum);
            } else {
                let total = TotalSupply::<T>::get(asset).unwrap_or_default();
                let new_total = <T as Config>::RuntimeFhe::sub(total, transferred.clone());
                <T as Config>::RuntimeFhe::allow_this(new_total.clone());
                TotalSupply::<T>::insert(asset, new_total);
            }

            if let Some(f) = from {
                <T as Config>::RuntimeFhe::allow_to::<T>(transferred.clone(), f);
            }
            if let Some(t) = to {
                <T as Config>::RuntimeFhe::allow_to::<T>(transferred.clone(), t);
            }
            <T as Config>::RuntimeFhe::allow_this(transferred.clone());

            Ok(transferred)
        }
    }
}
