use core::marker::PhantomData;

use frame_support::{
    parameter_types,
    traits::{ConstU32, Contains, ContainsPair, Everything, PalletInfoAccess},
    weights::Weight,
};
use orml_traits::{location::Reserve, parameter_type_with_key};
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use parity_scale_codec::{Decode, Encode};
use polkadot_parachain_primitives::primitives::{self, Sibling};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::Vec;
use xcm::latest::prelude::{Assets as XcmAssets, *};
use xcm_builder::{
    AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom, Case,
    ConvertedConcreteId, DenyReserveTransferToRelayChain, DenyThenTry, FixedWeightBounds,
    FungibleAdapter, FungiblesAdapter, HandleFee, IsChildSystemParachain, IsConcrete, NoChecking,
    ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
    SignedAccountKey20AsNative, SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId,
    WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
use xcm_executor::traits::{ConvertLocation, FeeReason, JustTry, TransactAsset};
use xcm_primitives::{AsAssetType, UtilityAvailableCalls, UtilityEncodeCall, XcmTransact};

use crate::{
    configs::{
        AssetType, Erc20XcmBridgePalletLocation, ParachainSystem, Runtime, RuntimeCall,
        RuntimeOrigin, XcmpQueue,
    },
    types::{AccountId, AssetId, Balance},
    AssetManager, Assets, Balances, ParachainInfo, Treasury,
};

parameter_types! {
    pub const RelayNetwork: Option<NetworkId> = None;
    pub AssetsPalletLocation: Location =
        PalletInstance(<Assets as PalletInfoAccess>::index() as u8).into();
    pub BalancesPalletLocation: Location = PalletInstance(<Balances as PalletInfoAccess>::index() as u8).into();
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
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

/// `AssetId/Balancer` converter for `TrustBackedAssets`
pub type TrustBackedAssetsConvertedConcreteId =
    assets_common::TrustBackedAssetsConvertedConcreteId<AssetsPalletLocation, Balance>;

/// Type for specifying how a `Location` can be converted into an
/// `AccountId`. This is used when determining ownership of accounts for asset
/// transacting and when attempting to use XCM `Transact` in order to determine
/// the dispatch Origin.
pub type LocationToAccountId = (
    // The parent (Relay-chain) origin converts to the parent `AccountId`.
    ParentIsPreset<AccountId>,
    // Sibling parachain origins convert to AccountId via the `ParaId::into`.
    SiblingParachainConvertsVia<Sibling, AccountId>,
    // If we receive a Location of type AccountKey20, just generate a native account
    AccountKey20Aliases<RelayNetwork, AccountId>,
);

/// Means for transacting native currency on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<BalancesPalletLocation>,
    // Do a simple punn to convert an AccountId20 Location into a native chain account ID:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports.
    (),
>;

/// Means for transacting assets besides the native currency on this chain.
pub type LocalFungiblesTransactor = FungiblesAdapter<
    // Use this fungibles implementation:
    Assets,
    // Use this currency when it is a fungible asset matching the given location or name:
    ConvertedConcreteId<AssetId, Balance, AsAssetType<AssetId, AssetType, AssetManager>, JustTry>,
    // Convert an XCM MultiLocation into a local account id:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports of `Assets`.
    NoChecking,
    // We don't track any teleports.
    (),
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
    // Xcm Origins defined by a Multilocation of type AccountKey20 can be converted to a 20 byte-
    // account local origin
    SignedAccountKey20AsNative<RelayNetwork, RuntimeOrigin>,
    // Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
    XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
    // One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
    pub const UnitWeightCost: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
    pub const MaxInstructions: u32 = 100;
    pub const MaxAssetsIntoHolding: u32 = 64;
}

pub struct ParentOrParentsExecutivePlurality;
impl Contains<Location> for ParentOrParentsExecutivePlurality {
    fn contains(location: &Location) -> bool {
        matches!(location.unpack(), (1, []) | (1, [Plurality { id: BodyId::Executive, .. }]))
    }
}

pub type Barrier = TrailingSetTopicAsId<
    DenyThenTry<
        DenyReserveTransferToRelayChain,
        (
            TakeWeightCredit,
            WithComputedOrigin<
                (
                    AllowTopLevelPaidExecutionFrom<Everything>,
                    AllowExplicitUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
                    // ^^^ Parent and its exec plurality get free execution
                ),
                UniversalLocation,
                ConstU32<8>,
            >,
        ),
    >,
>;

/// A `HandleFee` implementation that simply deposits the fees into a specific on-chain
/// `ReceiverAccount`.
///
/// It reuses the `AssetTransactor` configured on the XCM executor to deposit fee assets. If
/// the `AssetTransactor` returns an error while calling `deposit_asset`, then a warning will be
/// logged and the fee burned.
pub struct XcmFeeToAccount<AssetTransactor, AccountId, ReceiverAccount>(
    PhantomData<(AssetTransactor, AccountId, ReceiverAccount)>,
);
impl<
        AssetTransactor: TransactAsset,
        AccountId: Clone + Into<[u8; 20]>,
        ReceiverAccount: Get<AccountId>,
    > HandleFee for XcmFeeToAccount<AssetTransactor, AccountId, ReceiverAccount>
{
    fn handle_fee(fee: XcmAssets, context: Option<&XcmContext>, _reason: FeeReason) -> XcmAssets {
        deposit_or_burn_fee::<AssetTransactor, _>(fee, context, ReceiverAccount::get());

        XcmAssets::new()
    }
}

pub fn deposit_or_burn_fee<AssetTransactor: TransactAsset, AccountId: Clone + Into<[u8; 20]>>(
    fee: XcmAssets,
    context: Option<&XcmContext>,
    receiver: AccountId,
) {
    let dest = AccountKey20 { network: None, key: receiver.into() }.into();
    for asset in fee.into_inner() {
        if let Err(e) = AssetTransactor::deposit_asset(&asset, &dest, context) {
            log::trace!(
                target: "xcm::fees",
                "`AssetTransactor::deposit_asset` returned error: {:?}. Burning fee: {:?}. \
                They might be burned.",
                e, asset,
            );
        }
    }
}

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

parameter_types! {
    pub TreasuryAccount: AccountId = Treasury::account_id();
}

/// If you change this config, keep in mind that you should define how you collect fees.
pub type FeeManager = XcmFeeManagerFromComponents<
    IsChildSystemParachain<primitives::Id>,
    XcmFeeToAccount<AssetTransactors, AccountId, TreasuryAccount>,
>;
pub type XcmWeigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;

use frame_support::{pallet_prelude::Get, traits::OriginTrait};
use sp_runtime::traits::TryConvert;

// Convert a local Origin (i.e., a signed 20 byte account Origin)  to a Multilocation
pub struct SignedToAccountId20<Origin, AccountId, Network>(
    sp_std::marker::PhantomData<(Origin, AccountId, Network)>,
);
impl<Origin: OriginTrait + Clone, AccountId: Into<[u8; 20]>, Network: Get<Option<NetworkId>>>
    TryConvert<Origin, Location> for SignedToAccountId20<Origin, AccountId, Network>
where
    Origin::PalletsOrigin: From<frame_system::RawOrigin<AccountId>>
        + TryInto<frame_system::RawOrigin<AccountId>, Error = Origin::PalletsOrigin>,
{
    fn try_convert(o: Origin) -> Result<Location, Origin> {
        o.try_with_caller(|caller| match caller.try_into() {
            Ok(frame_system::RawOrigin::Signed(who)) =>
                Ok(AccountKey20 { key: who.into(), network: Network::get() }.into()),
            Ok(other) => Err(other.into()),
            Err(other) => Err(other),
        })
    }
}

// Converts a Signed Local Origin into a Location
pub type LocalOriginToLocation = SignedToAccountId20<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into
/// the right message queues.
pub type XcmRouter = WithUniqueTopic<(
    // Two routers - use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, (), ()>,
    // ..and XCMP to communicate with the sibling chains.
    XcmpQueue,
)>;

// We are not using all of these below atm, but we will need them when configuring `orml_xtokens`
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
    // Erc20 token
    Erc20 { contract_address: H160 },
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
            CurrencyId::Erc20 { contract_address } => {
                let mut location = Erc20XcmBridgePalletLocation::get();
                location
                    .push_interior(Junction::AccountKey20 {
                        key: contract_address.0,
                        network: None,
                    })
                    .ok();
                Some(location)
            }
        }
    }
}

/// Wrapper type around `LocationToAccountId` to convert an `AccountId` to type `H160`.
pub struct LocationToH160;
impl ConvertLocation<H160> for LocationToH160 {
    fn convert_location(location: &Location) -> Option<H160> {
        <LocationToAccountId as ConvertLocation<AccountId>>::convert_location(location)
            .map(Into::into)
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
// removed ORML dependencies
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

/// Struct that uses RelativeReserveProvider to output relative views of multilocations
///
/// Additionally accepts a Location that aims at representing the chain part
/// (parent: 1, Parachain(paraId)) of the absolute representation of our chain.
/// If a token reserve matches against this absolute view, we return  Some(Location::here())
/// This helps users by preventing errors when they try to transfer a token through xtokens
/// to our chain (either inserting the relative or the absolute value).
// Moved from Moonbeam to implement orml_traits::location::Reserve after Moonbeam
// removed ORML dependencies
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
            && location.first_interior() != Erc20XcmBridgePalletLocation::get().first_interior()
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

    use super::*;

    /// This From exists for benchmarking purposes. It has the potential side-effect of calling
    /// AssetManager::set_asset_type_asset_id() and should NOT be used in any production code.
    impl From<Location> for CurrencyId {
        fn from(location: Location) -> CurrencyId {
            use xcm_primitives::AssetTypeGetter;

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

    mod is_bridged_concrete_assets_from {
        use frame_support::traits::ContainsPair;
        use xcm::latest::{Asset, AssetId, Fungibility, Junctions, Location};

        use crate::configs::{AssetHubLocation, IsBridgedConcreteAssetFrom};

        #[test]
        pub fn is_bridged_concrete_assets_from_contains() {
            let asset = Asset {
                id: AssetId(Location { parents: 2, interior: Junctions::Here }),
                fun: Fungibility::Fungible(100),
            };
            assert!(IsBridgedConcreteAssetFrom::<AssetHubLocation>::contains(
                &asset,
                &AssetHubLocation::get()
            ))
        }

        #[test]
        pub fn is_bridged_concrete_assets_from_not_contains() {
            let asset = Asset {
                id: AssetId(Location { parents: 1, interior: Junctions::Here }),
                fun: Fungibility::Fungible(100),
            };
            assert!(!IsBridgedConcreteAssetFrom::<AssetHubLocation>::contains(
                &asset,
                &AssetHubLocation::get()
            ))
        }
    }

    mod location_conversion {
        use sp_core::H160;
        use sp_runtime::traits::{Convert, TryConvert};
        use xcm::latest::{Junction::AccountKey20, Location};

        use crate::{
            configs::{
                CurrencyId, CurrencyIdToLocation, RelayNetwork, SelfReserve, SignedToAccountId20,
            },
            types::AccountId,
            RuntimeOrigin,
        };

        #[test]
        fn test_account_id_to_location_conversion() {
            let account_id: AccountId = H160::from_low_u64_be(1).into();
            let expected_location =
                Location::new(0, AccountKey20 { network: None, key: account_id.into() });

            let origin = RuntimeOrigin::signed(account_id);
            let result = SignedToAccountId20::<_, AccountId, RelayNetwork>::try_convert(origin);

            assert_eq!(result.unwrap(), expected_location); // Unwrap ensures proper equality
        }

        #[test]
        fn test_currency_id_to_location_self_reserve() {
            let currency = CurrencyId::SelfReserve;
            let expected_location = SelfReserve::get();

            let result = CurrencyIdToLocation::<()>::convert(currency);

            assert_eq!(result, Some(expected_location));
        }

        #[test]
        fn test_currency_id_to_location_erc20() {
            let contract_address = H160::from_low_u64_be(0x1234);
            let currency = CurrencyId::Erc20 { contract_address };

            let mut expected_location = crate::configs::Erc20XcmBridgePalletLocation::get();
            expected_location
                .push_interior(AccountKey20 { network: None, key: contract_address.0 })
                .unwrap();

            let result = CurrencyIdToLocation::<()>::convert(currency);

            assert_eq!(result, Some(expected_location));
        }
    }

    mod asset_fee_filter {
        use frame_support::traits::Contains;
        use xcm::latest::{Junction::AccountKey20, Location};

        use crate::configs::AssetFeesFilter;

        #[test]
        fn test_asset_fee_filter_valid() {
            let location = Location::new(1, AccountKey20 { network: None, key: [0u8; 20] });
            assert!(AssetFeesFilter::contains(&location));
        }

        #[test]
        fn test_asset_fee_filter_invalid() {
            let location = crate::configs::Erc20XcmBridgePalletLocation::get();
            assert!(!AssetFeesFilter::contains(&location));
        }

        #[test]
        fn test_asset_fee_filter_no_parents() {
            let location = Location::new(0, AccountKey20 { network: None, key: [0u8; 20] });
            assert!(!AssetFeesFilter::contains(&location));
        }
    }

    mod asset_transactor {
        use xcm::latest::Location;
        use xcm_primitives::{UtilityAvailableCalls, UtilityEncodeCall, XcmTransact};

        use crate::configs::Transactors;

        #[test]
        fn test_transactors_destination_relay() {
            let transactor = Transactors::Relay;
            let destination = transactor.destination();
            assert_eq!(destination, Location::parent());
        }

        #[test]
        fn test_transactors_encode_call() {
            sp_io::TestExternalities::default().execute_with(|| {
                let transactor = Transactors::Relay;
                let call = UtilityAvailableCalls::AsDerivative(0, vec![]);
                let encoded = transactor.encode_call(call);
                assert!(!encoded.is_empty());
            });
        }

        #[test]
        fn test_transactors_try_from_valid() {
            let transactor = Transactors::try_from(0u8);
            assert!(transactor.is_ok());
        }

        #[test]
        fn test_transactors_try_from_invalid() {
            let transactor = Transactors::try_from(1u8);
            assert!(transactor.is_err());
        }
    }

    mod reserve_providers {
        use frame_support::traits::Get;
        use orml_traits::location::Reserve;
        use xcm::latest::{Asset, AssetId, Fungibility, Junction, Junctions, Location};

        use crate::configs::{
            AbsoluteAndRelativeReserve, RelativeReserveProvider, SelfLocationAbsolute,
        };

        #[test]
        fn test_relative_reserve_provider_here() {
            // Test case for a local asset (parents = 0, no Parachain junction)
            let local_asset = Asset {
                id: AssetId(Location { parents: 0, interior: Junctions::Here }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = RelativeReserveProvider::reserve(&local_asset);
            assert_eq!(reserve, Some(Location::here()));
        }

        #[test]
        fn test_relative_reserve_provider_local_with_junctions() {
            // Test case for a local asset with junctions but not Parachain
            use std::sync::Arc;

            let junction = Junction::AccountId32 { network: None, id: [0; 32] };
            let junction_array = Arc::new([junction]);

            let local_asset_with_junctions = Asset {
                id: AssetId(Location { parents: 0, interior: Junctions::X1(junction_array) }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = RelativeReserveProvider::reserve(&local_asset_with_junctions);
            assert_eq!(reserve, Some(Location::here()));
        }

        #[test]
        fn test_relative_reserve_provider_parachain() {
            // Test case for a parachain asset
            use std::sync::Arc;

            let junction = Junction::Parachain(1000);
            let junction_array = Arc::new([junction]);

            let parachain_asset = Asset {
                id: AssetId(Location {
                    parents: 0,
                    interior: Junctions::X1(junction_array.clone()),
                }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = RelativeReserveProvider::reserve(&parachain_asset);
            // Should return the chain location, not Here
            let expected = Location { parents: 0, interior: Junctions::X1(junction_array) };
            assert_eq!(reserve, Some(expected));
        }

        #[test]
        fn test_relative_reserve_provider_parent() {
            // Test case for a parent asset
            let parent_asset = Asset {
                id: AssetId(Location { parents: 1, interior: Junctions::Here }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = RelativeReserveProvider::reserve(&parent_asset);
            // Should return the chain location
            let location = parent_asset.id.0.clone();
            assert_eq!(reserve, Some(location));
        }

        #[test]
        fn test_absolute_and_relative_reserve_matches_absolute() {
            // Create a mock implementation of AbsoluteMultiLocation
            use std::sync::Arc;

            struct MockAbsoluteLocation;
            impl Get<Location> for MockAbsoluteLocation {
                fn get() -> Location {
                    let junction = Junction::Parachain(1000);
                    let junction_array = Arc::new([junction]);
                    Location { parents: 1, interior: Junctions::X1(junction_array) }
                }
            }

            // Test case where the reserve matches the absolute location
            let junction = Junction::Parachain(1000);
            let junction_array = Arc::new([junction]);

            let asset = Asset {
                id: AssetId(Location { parents: 1, interior: Junctions::X1(junction_array) }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = AbsoluteAndRelativeReserve::<MockAbsoluteLocation>::reserve(&asset);
            // Should return Here since it matches the absolute location
            assert_eq!(reserve, Some(Location::here()));
        }

        #[test]
        fn test_absolute_and_relative_reserve_different_location() {
            // Create a mock implementation of AbsoluteMultiLocation
            use std::sync::Arc;

            struct MockAbsoluteLocation;
            impl Get<Location> for MockAbsoluteLocation {
                fn get() -> Location {
                    let junction = Junction::Parachain(1000);
                    let junction_array = Arc::new([junction]);
                    Location { parents: 1, interior: Junctions::X1(junction_array) }
                }
            }

            // Test case where the reserve doesn't match the absolute location
            let junction = Junction::Parachain(2000);
            let junction_array = Arc::new([junction]);

            let asset = Asset {
                id: AssetId(Location { parents: 1, interior: Junctions::X1(junction_array) }),
                fun: Fungibility::Fungible(100),
            };

            let reserve = AbsoluteAndRelativeReserve::<MockAbsoluteLocation>::reserve(&asset);
            // Should return the relative reserve since it doesn't match the absolute location
            let expected = Location { parents: 1, interior: Junctions::X1(Arc::new([junction])) };
            assert_eq!(reserve, Some(expected));
        }

        #[test]
        fn test_absolute_and_relative_reserve_with_self_location() {
            // We need to use TestExternalities for this test
            sp_io::TestExternalities::default().execute_with(|| {
                // Test with the actual SelfLocationAbsolute
                let self_location = SelfLocationAbsolute::get();
                let asset =
                    Asset { id: AssetId(self_location.clone()), fun: Fungibility::Fungible(100) };

                let reserve = AbsoluteAndRelativeReserve::<SelfLocationAbsolute>::reserve(&asset);
                // Should return Here since it matches the self location
                assert_eq!(reserve, Some(Location::here()));
            });
        }
    }
}
