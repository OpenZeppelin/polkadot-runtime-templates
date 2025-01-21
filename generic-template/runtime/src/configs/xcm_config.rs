use core::marker::PhantomData;

use frame_support::{
    parameter_types,
    traits::{ContainsPair, Get, PalletInfoAccess},
    weights::Weight,
};
use orml_traits::{location::Reserve, parameter_type_with_key};
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use parity_scale_codec::{Decode, Encode};
use polkadot_parachain_primitives::primitives::{self, Sibling};
use scale_info::TypeInfo;
use sp_runtime::{traits::Convert, Vec};
use xcm::latest::{prelude::*, Junction::PalletInstance};
use xcm_builder::{
    AccountId32Aliases, Case, FixedWeightBounds, FungibleAdapter, FungiblesAdapter,
    IsChildSystemParachain, IsConcrete, NoChecking, ParentIsPreset, RelayChainAsNative,
    SendXcmFeeToAccount, SiblingParachainAsNative, SiblingParachainConvertsVia,
    SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, WithUniqueTopic,
    XcmFeeManagerFromComponents,
};
use xcm_primitives::{UtilityAvailableCalls, UtilityEncodeCall, XcmTransact};

use crate::{
    configs::{MaxInstructions, ParachainSystem, Runtime, RuntimeCall, UnitWeightCost, XcmpQueue},
    types::{AccountId, AssetId, Balance, TreasuryAccount},
    Assets, Balances, ParachainInfo, PolkadotXcm, RuntimeOrigin,
};

parameter_types! {
    pub PlaceholderAccount: AccountId = PolkadotXcm::check_account();
    pub const RelayNetwork: Option<NetworkId> = None;
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
    pub AssetsPalletLocation: Location =
        PalletInstance(<Assets as PalletInfoAccess>::index() as u8).into();
    pub UniversalLocation: InteriorLocation = Parachain(ParachainInfo::parachain_id().into()).into();
    // Self Reserve location, defines the multilocation identifiying the self-reserve currency
    // This is used to match it also against our Balances pallet when we receive such
    // a Location: (Self Balances pallet index)
    // We use the RELATIVE multilocation
    pub SelfReserve: Location = Location {
        parents:0,
        interior: [
            PalletInstance(<Balances as PalletInfoAccess>::index() as u8)
        ].into()
    };
}

/// Type for specifying how a `Location` can be converted into an
/// `AccountId`. This is used when determining ownership of accounts for asset
/// transacting and when attempting to use XCM `Transact` in order to determine
/// the dispatch Origin.
pub type LocationToAccountId = (
    // The parent (Relay-chain) origin converts to the parent `AccountId`.
    ParentIsPreset<AccountId>,
    // Sibling parachain origins convert to AccountId via the `ParaId::into`.
    SiblingParachainConvertsVia<Sibling, AccountId>,
    // Straight up local `AccountId32` origins just alias directly to `AccountId`.
    AccountId32Aliases<RelayNetwork, AccountId>,
);

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<RelayLocation>,
    // Do a simple punn to convert an AccountId32 Location into a native chain account ID:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports.
    (),
>;

/// `AssetId/Balancer` converter for `TrustBackedAssets`
pub type TrustBackedAssetsConvertedConcreteId =
    assets_common::TrustBackedAssetsConvertedConcreteId<AssetsPalletLocation, Balance>;

/// Means for transacting assets besides the native currency on this chain.
pub type LocalFungiblesTransactor = FungiblesAdapter<
    // Use this fungibles implementation:
    Assets,
    // Use this currency when it is a fungible asset matching the given location or name:
    TrustBackedAssetsConvertedConcreteId,
    // Convert an XCM MultiLocation into a local account id:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports of `Assets`.
    NoChecking,
    // We don't track any teleports of `Assets`, but a placeholder account is provided due to trait
    // bounds.
    PlaceholderAccount,
>;

/// Means for transacting assets on this chain.
pub type AssetTransactors = (LocalAssetTransactor, LocalFungiblesTransactor);

/// This is the type we use to convert an (incoming) XCM origin into a local
/// `Origin` instance, ready for dispatching a transaction with Xcm's
/// `Transact`. There is an `OriginKind` which can biases the kind of local
/// `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
    // Sovereign account converter; this attempts to derive an `AccountId` from the origin location
    // using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
    // foreign chains who want to have a local sovereign account on this chain which they control.
    SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
    // Native converter for Relay-chain (Parent) location; will convert to a `Relay` origin when
    // recognized.
    RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
    // Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
    // recognized.
    SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
    // Native signed account converter; this just converts an `AccountId32` origin into a normal
    // `RuntimeOrigin::Signed` origin of the same 32-byte value.
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    // Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
    XcmPassthrough<RuntimeOrigin>,
);

pub type FeeManager = XcmFeeManagerFromComponents<
    IsChildSystemParachain<primitives::Id>,
    SendXcmFeeToAccount<AssetTransactors, TreasuryAccount>,
>;

/// Matches foreign assets from a given origin.
/// Foreign assets are assets bridged from other consensus systems. i.e parents > 1.
pub struct IsBridgedConcreteAssetFrom<Origin>(PhantomData<Origin>);
impl<Origin> ContainsPair<Asset, Location> for IsBridgedConcreteAssetFrom<Origin>
where
    Origin: Get<Location>,
{
    fn contains(asset: &Asset, origin: &Location) -> bool {
        let loc = Origin::get();
        &loc == origin
            && matches!(
                asset,
                Asset { id: AssetId(Location { parents: 2, .. }), fun: Fungibility::Fungible(_) },
            )
    }
}

parameter_types! {
    /// Location of Asset Hub
   pub AssetHubLocation: Location = Location::new(1, [Parachain(1000)]);
   pub const RelayLocation: Location = Location::parent();
   pub RelayLocationFilter: AssetFilter = Wild(AllOf {
       fun: WildFungible,
       id: xcm::prelude::AssetId(RelayLocation::get()),
   });
   pub RelayChainNativeAssetFromAssetHub: (AssetFilter, Location) = (
       RelayLocationFilter::get(),
       AssetHubLocation::get()
   );
}

pub type Reserves = (
    // Assets bridged from different consensus systems held in reserve on Asset Hub.
    IsBridgedConcreteAssetFrom<AssetHubLocation>,
    // Relaychain (DOT) from Asset Hub
    Case<RelayChainNativeAssetFromAssetHub>,
    // Assets which the reserve is the same as the origin.
    MultiNativeAsset<AbsoluteAndRelativeReserve<SelfLocationAbsolute>>,
);

pub type XcmWeigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into
/// the right message queues.
pub type XcmRouter = WithUniqueTopic<(
    // Two routers - use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, (), ()>,
    // ..and XCMP to communicate with the sibling chains.
    XcmpQueue,
)>;

parameter_types! {
    pub const MaxLockers: u32 = 8;
    pub const MaxRemoteLockConsumers: u32 = 0;
}

parameter_types! {
    pub const BaseXcmWeight: Weight = Weight::from_parts(200_000_000u64, 0);
    pub const MaxAssetsForTransfer: usize = 2;

    // This is how we are going to detect whether the asset is a Reserve asset
    // This however is the chain part only
    pub SelfLocation: Location = Location::here();
    // We need this to be able to catch when someone is trying to execute a non-
    // cross-chain transfer in xtokens through the absolute path way
    pub SelfLocationAbsolute: Location = Location {
        parents:1,
        interior: [
            Parachain(ParachainInfo::parachain_id().into())
        ].into()
    };
}

parameter_type_with_key! {
    pub ParachainMinFee: |location: Location| -> Option<u128> {
        match (location.parents, location.first_interior()) {
            // Polkadot AssetHub fee
            (1, Some(Parachain(1000u32))) => Some(50_000_000u128),
            _ => None,
        }
    };
}

// Our currencyId. We distinguish for now between SelfReserve, and Others, defined by their Id.
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum CurrencyId {
    // Our native token
    SelfReserve,
    // Assets representing other chains native tokens
    ForeignAsset(AssetId),
}

parameter_types! {
    pub PolkadotXcmLocation: Location = Location {
        parents:0,
        interior: [
            PalletInstance(<PolkadotXcm as PalletInfoAccess>::index() as u8)
        ].into()
    };
}

// How to convert from CurrencyId to Location
pub struct CurrencyIdToLocation<AssetXConverter>(sp_std::marker::PhantomData<AssetXConverter>);
impl<AssetXConverter> sp_runtime::traits::Convert<CurrencyId, Option<Location>>
    for CurrencyIdToLocation<AssetXConverter>
where
    AssetXConverter: sp_runtime::traits::MaybeEquivalence<Location, AssetId>,
{
    fn convert(currency: CurrencyId) -> Option<Location> {
        match currency {
            CurrencyId::SelfReserve => {
                let multi: Location = SelfReserve::get();
                Some(multi)
            }
            CurrencyId::ForeignAsset(asset) => AssetXConverter::convert_back(&asset),
        }
    }
}

pub struct AccountIdToLocation;
impl Convert<AccountId, Location> for AccountIdToLocation {
    fn convert(account: AccountId) -> Location {
        AccountId32 { network: None, id: account.into() }.into()
    }
}

/// The `DOTReserveProvider` overrides the default reserve location for DOT (Polkadot's native token).
///
/// DOT can exist in multiple locations, and this provider ensures that the reserve is correctly set
/// to the AssetHub parachain.
///
/// - **Default Location:** `{ parents: 1, location: Here }`
/// - **Reserve Location on AssetHub:** `{ parents: 1, location: X1(Parachain(AssetHubParaId)) }`
///
/// This provider ensures that if the asset's ID points to the default "Here" location,
/// it will instead be mapped to the AssetHub parachain.
pub struct DOTReserveProvider;

impl Reserve for DOTReserveProvider {
    fn reserve(asset: &Asset) -> Option<Location> {
        let AssetId(location) = &asset.id;

        let dot_here = Location::new(1, Here);
        let dot_asset_hub = AssetHubLocation::get();

        if location == &dot_here {
            Some(dot_asset_hub) // Reserve is on AssetHub.
        } else {
            None
        }
    }
}

/// The `BridgedAssetReserveProvider` handles assets that are bridged from external consensus systems
/// (e.g., Ethereum) and may have multiple valid reserve locations.
///
/// Specifically, these bridged assets can have two known reserves:
/// 1. **Ethereum-based Reserve:**
///    `{ parents: 1, location: X1(GlobalConsensus(Ethereum{ chain_id: 1 })) }`
/// 2. **AssetHub Parachain Reserve:**
///    `{ parents: 1, location: X1(Parachain(AssetHubParaId)) }`
///
/// This provider maps the reserve for bridged assets to AssetHub when the asset originates
/// from a global consensus system, such as Ethereum.
pub struct BridgedAssetReserveProvider;

impl Reserve for BridgedAssetReserveProvider {
    fn reserve(asset: &Asset) -> Option<Location> {
        let AssetId(location) = &asset.id;

        let asset_hub_reserve = AssetHubLocation::get();

        // any asset that has parents > 1 and interior that starts with GlobalConsensus(_) pattern
        // can be considered a bridged asset.
        //
        // `split_global` will return an `Err` if the first item is not a `GlobalConsensus`
        if location.parents > 1 && location.interior.clone().split_global().is_ok() {
            Some(asset_hub_reserve)
        } else {
            None
        }
    }
}

// Provide reserve in relative path view
// Self tokens are represeneted as Here
// Moved from Moonbeam to implement orml_traits::location::Reserve after Moonbeam
// removed ORML dependenies
pub struct RelativeReserveProvider;

impl Reserve for RelativeReserveProvider {
    fn reserve(asset: &Asset) -> Option<Location> {
        let AssetId(location) = &asset.id;
        if location.parents == 0
            && !matches!(location.first_interior(), Some(Junction::Parachain(_)))
        {
            Some(Location::here())
        } else {
            Some(location.chain_location())
        }
    }
}

/// This struct offers uses RelativeReserveProvider to output relative views of multilocations
/// However, additionally accepts a Location that aims at representing the chain part
/// (parent: 1, Parachain(paraId)) of the absolute representation of our chain.
/// If a token reserve matches against this absolute view, we return  Some(Location::here())
/// This helps users by preventing errors when they try to transfer a token through xtokens
/// to our chain (either inserting the relative or the absolute value).
// Moved from Moonbeam to implement orml_traits::location::Reserve after Moonbeam
// removed ORML dependenies
pub struct AbsoluteAndRelativeReserve<AbsoluteMultiLocation>(PhantomData<AbsoluteMultiLocation>);
impl<AbsoluteMultiLocation> Reserve for AbsoluteAndRelativeReserve<AbsoluteMultiLocation>
where
    AbsoluteMultiLocation: Get<Location>,
{
    fn reserve(asset: &Asset) -> Option<Location> {
        RelativeReserveProvider::reserve(asset).map(|relative_reserve| {
            if relative_reserve == AbsoluteMultiLocation::get() {
                Location::here()
            } else {
                relative_reserve
            }
        })
    }
}

pub struct ReserveProviders;

impl Reserve for ReserveProviders {
    fn reserve(asset: &Asset) -> Option<Location> {
        // Try each provider's reserve method in sequence.
        DOTReserveProvider::reserve(asset)
            .or_else(|| BridgedAssetReserveProvider::reserve(asset))
            .or_else(|| AbsoluteAndRelativeReserve::<SelfLocationAbsolute>::reserve(asset))
    }
}

pub struct AssetFeesFilter;
impl frame_support::traits::Contains<Location> for AssetFeesFilter {
    fn contains(location: &Location) -> bool {
        location.parent_count() > 0
            && location.first_interior() != PolkadotXcmLocation::get().first_interior()
    }
}

// For now we only allow to transact in the relay, although this might change in the future
// Transactors just defines the chains in which we allow transactions to be issued through
// xcm
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum Transactors {
    Relay,
}

// Default for benchmarking
#[cfg(feature = "runtime-benchmarks")]
impl Default for Transactors {
    fn default() -> Self {
        Transactors::Relay
    }
}

impl TryFrom<u8> for Transactors {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0u8 => Ok(Transactors::Relay),
            _ => Err(()),
        }
    }
}

impl UtilityEncodeCall for Transactors {
    fn encode_call(self, call: UtilityAvailableCalls) -> Vec<u8> {
        match self {
            Transactors::Relay => pallet_xcm_transactor::Pallet::<Runtime>::encode_call(
                pallet_xcm_transactor::Pallet(sp_std::marker::PhantomData::<Runtime>),
                call,
            ),
        }
    }
}

impl XcmTransact for Transactors {
    fn destination(self) -> Location {
        match self {
            Transactors::Relay => Location::parent(),
        }
    }
}

parameter_types! {
    pub MaxHrmpRelayFee: Asset = (Location::parent(), 1_000_000_000_000u128).into();
}

#[cfg(feature = "runtime-benchmarks")]
mod testing {
    use sp_runtime::traits::MaybeEquivalence;
    use xcm_builder::WithLatestLocationConverter;
    use xcm_executor::traits::ConvertLocation;

    use super::*;

    /// This From exists for benchmarking purposes. It has the potential side-effect of calling
    /// AssetManager::set_asset_type_asset_id() and should NOT be used in any production code.
    impl From<Location> for CurrencyId {
        fn from(location: Location) -> CurrencyId {
            use xcm_primitives::{AsAssetType, AssetTypeGetter};

            use crate::{configs::asset_config::AssetType, AssetManager};

            // If it does not exist, for benchmarking purposes, we create the association
            let asset_id = if let Some(asset_id) =
                AsAssetType::<AssetId, AssetType, AssetManager>::convert_location(&location)
            {
                asset_id
            } else {
                let asset_type = AssetType::Xcm(
                    WithLatestLocationConverter::convert(&location).expect("convert to v3"),
                );
                let asset_id: AssetId = asset_type.clone().into();
                AssetManager::set_asset_type_asset_id(asset_type, asset_id);
                asset_id
            };

            CurrencyId::ForeignAsset(asset_id)
        }
    }
}

#[cfg(test)]
mod tests {
    mod location_conversion {
        use sp_runtime::traits::Convert;
        use xcm::latest::{Junction::AccountId32, Location};

        use crate::configs::{AccountIdToLocation, CurrencyId, CurrencyIdToLocation, SelfReserve};

        #[test]
        fn test_account_id_to_location_conversion() {
            let account_id = [1u8; 32];
            let expected_location = Location::new(0, AccountId32 { network: None, id: account_id });

            let result = AccountIdToLocation::convert(account_id.into());

            assert_eq!(result, expected_location);
        }

        #[test]
        fn test_account_id_to_location_empty() {
            let account_id = [0u8; 32];
            let expected_location = Location::new(0, AccountId32 { network: None, id: account_id });

            let result = AccountIdToLocation::convert(account_id.into());

            assert_eq!(result, expected_location);
        }

        #[test]
        fn test_account_id_to_location_unusual_bytes() {
            let account_id = [0xFF; 32];
            let expected_location = Location::new(0, AccountId32 { network: None, id: account_id });

            let result = AccountIdToLocation::convert(account_id.into());

            assert_eq!(result, expected_location);
        }

        #[test]
        fn test_currency_id_to_location_self_reserve() {
            let currency = CurrencyId::SelfReserve;
            let expected_location = SelfReserve::get();

            let result = CurrencyIdToLocation::<()>::convert(currency);

            assert_eq!(result, Some(expected_location));
        }

        #[test]
        fn test_currency_id_to_location_foreign_asset() {
            let foreign_asset_id = 123u32;
            let currency = CurrencyId::ForeignAsset(foreign_asset_id);

            let result = CurrencyIdToLocation::<()>::convert(currency);

            assert!(result.is_none()); // As no `convert_back` logic is defined in the placeholder.
        }
    }

    mod asset_fee_filter {
        use cumulus_primitives_core::Junctions::{X1, X4};
        use frame_support::traits::Contains;
        use sp_std::sync::Arc;
        use xcm::{latest::Location, prelude::*};

        use crate::configs::{AssetFeesFilter, PolkadotXcmLocation};

        #[test]
        fn test_asset_fee_filter_valid() {
            let location = Location::new(1, X1(Arc::new([Junction::Parachain(1000)])));
            assert!(AssetFeesFilter::contains(&location));
        }

        #[test]
        fn test_asset_fee_filter_invalid() {
            let invalid_location = PolkadotXcmLocation::get();
            assert!(!AssetFeesFilter::contains(&invalid_location));
        }

        #[test]
        fn test_asset_fee_filter_no_parents() {
            let location = Location::new(0, Here);
            assert!(!AssetFeesFilter::contains(&location));
        }

        #[test]
        fn test_asset_fee_filter_deeply_nested_junctions() {
            let location = Location::new(
                1,
                X4(Arc::new([
                    Junction::Parachain(1000),
                    Junction::PalletInstance(5),
                    Junction::GeneralIndex(42),
                    Junction::AccountId32 { network: None, id: [1u8; 32] },
                ])),
            );
            assert!(AssetFeesFilter::contains(&location));
        }

        #[test]
        fn test_asset_fee_filter_unusual_parents() {
            let location = Location::new(5, X1(Arc::new([Junction::Parachain(1000)])));
            assert!(AssetFeesFilter::contains(&location));
        }
    }

    mod asset_transactor {
        use xcm::latest::Location;
        use xcm_primitives::{UtilityAvailableCalls, UtilityEncodeCall, XcmTransact};

        use crate::configs::Transactors;

        #[test]
        fn test_transactors_destination_relay() {
            let transactor = Transactors::Relay;
            let expected_location = Location::parent();

            let result = transactor.destination();

            assert_eq!(result, expected_location);
        }

        #[test]
        fn test_transactors_encode_call() {
            sp_io::TestExternalities::default().execute_with(|| {
                let transactor = Transactors::Relay;
                let call = UtilityAvailableCalls::AsDerivative(1u16, Vec::<u8>::new());

                let encoded_call = transactor.encode_call(call);
                assert!(!encoded_call.is_empty()); // Ensure the call encodes successfully.
            });
        }

        #[test]
        fn test_transactors_try_from_valid() {
            let result = Transactors::try_from(0u8);
            assert_eq!(result, Ok(Transactors::Relay));
        }

        #[test]
        fn test_transactors_try_from_invalid() {
            let result = Transactors::try_from(1u8);
            assert!(result.is_err());
        }
    }
}
