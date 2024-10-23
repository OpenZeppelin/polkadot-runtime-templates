use core::marker::PhantomData;

use frame_support::{
    pallet_prelude::Get,
    parameter_types,
    traits::{ConstU32, Contains, ContainsPair, Everything, Nothing, PalletInfoAccess},
    weights::Weight,
};
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use parity_scale_codec::{Decode, Encode};
use polkadot_parachain_primitives::primitives::{self, Sibling};
use scale_info::TypeInfo;
use sp_runtime::traits::Convert;
use xcm::latest::prelude::*;
use xcm_builder::{
    AccountId32Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom, Case,
    DenyReserveTransferToRelayChain, DenyThenTry, EnsureXcmOrigin, FixedWeightBounds,
    FrameTransactionalProcessor, FungibleAdapter, FungiblesAdapter, IsChildSystemParachain,
    IsConcrete, NoChecking, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative,
    SiblingParachainConvertsVia, SignedAccountId32AsNative, SignedToAccountId32,
    SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId, WithComputedOrigin,
    WithUniqueTopic, XcmFeeManagerFromComponents, XcmFeeToAccount,
};
use xcm_executor::XcmExecutor;
use xcm_primitives::{AbsoluteAndRelativeReserve, AccountIdToCurrencyId, AsAssetType};

use super::TreasuryAccount;
use crate::{
    configs::{
        asset_config::AccountIdAssetIdConversion, weights, AssetType, Balances, ParachainSystem,
        Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, XcmpQueue,
    },
    types::{AccountId, AssetId, Balance},
    AllPalletsWithSystem, AssetManager, Assets, ParachainInfo, PolkadotXcm,
};

parameter_types! {
    pub const RelayNetwork: Option<NetworkId> = None;
    pub PlaceholderAccount: AccountId = PolkadotXcm::check_account();
    pub AssetsPalletLocation: Location =
        PalletInstance(<Assets as PalletInfoAccess>::index() as u8).into();
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

type Reserves = (
    // Assets bridged from different consensus systems held in reserve on Asset Hub.
    IsBridgedConcreteAssetFrom<AssetHubLocation>,
    // Relaychain (DOT) from Asset Hub
    Case<RelayChainNativeAssetFromAssetHub>,
    // Assets which the reserve is the same as the origin.
    MultiNativeAsset<AbsoluteAndRelativeReserve<SelfLocationAbsolute>>,
);

pub type XcmWeigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type Aliasers = Nothing;
    type AssetClaims = PolkadotXcm;
    type AssetExchanger = ();
    type AssetLocker = ();
    // How to withdraw and deposit an asset.
    type AssetTransactor = AssetTransactors;
    type AssetTrap = PolkadotXcm;
    type Barrier = Barrier;
    type CallDispatcher = RuntimeCall;
    /// When changing this config, keep in mind, that you should collect fees.
    type FeeManager = XcmFeeManagerFromComponents<
        IsChildSystemParachain<primitives::Id>,
        XcmFeeToAccount<Self::AssetTransactor, AccountId, TreasuryAccount>,
    >;
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type HrmpNewChannelOpenRequestHandler = ();
    /// Please, keep these two configs (`IsReserve` and `IsTeleporter`) mutually exclusive
    type IsReserve = Reserves;
    type IsTeleporter = ();
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type MessageExporter = ();
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type ResponseHandler = PolkadotXcm;
    type RuntimeCall = RuntimeCall;
    type SafeCallFilter = Everything;
    type SubscriptionService = PolkadotXcm;
    type Trader = pallet_xcm_weight_trader::Trader<Runtime>;
    type TransactionalProcessor = FrameTransactionalProcessor;
    type UniversalAliases = Nothing;
    // Teleporting is disabled.
    type UniversalLocation = UniversalLocation;
    type Weigher = XcmWeigher;
    type XcmRecorder = PolkadotXcm;
    type XcmSender = XcmRouter;
}

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

impl pallet_xcm::Config for Runtime {
    type AdminOrigin = EnsureRoot<AccountId>;
    // ^ Override for AdvertisedXcmVersion default
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type MaxLockers = MaxLockers;
    type MaxRemoteLockConsumers = MaxRemoteLockConsumers;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = LocationToAccountId;
    type TrustedLockers = ();
    type UniversalLocation = UniversalLocation;
    type Weigher = XcmWeigher;
    /// Rerun benchmarks if you are making changes to runtime configuration.
    type WeightInfo = weights::pallet_xcm::WeightInfo<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type XcmExecuteFilter = Everything;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type XcmExecuteFilter = Nothing;
    // Needs to be `Everything` for local testing.
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmReserveTransferFilter = Everything;
    type XcmRouter = XcmRouter;
    type XcmTeleportFilter = Nothing;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

impl cumulus_pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
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

impl AccountIdToCurrencyId<AccountId, CurrencyId> for Runtime {
    fn account_to_currency_id(account: AccountId) -> Option<CurrencyId> {
        Runtime::account_to_asset_id(account)
            .map(|(_prefix, asset_id)| CurrencyId::ForeignAsset(asset_id))
    }
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

impl orml_xtokens::Config for Runtime {
    type AccountIdToLocation = AccountIdToLocation;
    type Balance = Balance;
    type BaseXcmWeight = BaseXcmWeight;
    type CurrencyId = CurrencyId;
    type CurrencyIdConvert = CurrencyIdToLocation<AsAssetType<AssetId, AssetType, AssetManager>>;
    type LocationsFilter = Everything;
    type MaxAssetsForTransfer = MaxAssetsForTransfer;
    type MinXcmFee = ParachainMinFee;
    type RateLimiter = ();
    type RateLimiterId = ();
    type ReserveProvider = AbsoluteAndRelativeReserve<SelfLocationAbsolute>;
    type RuntimeEvent = RuntimeEvent;
    type SelfLocation = SelfLocation;
    type UniversalLocation = UniversalLocation;
    type Weigher = XcmWeigher;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

pub struct AssetFeesFilter;
impl frame_support::traits::Contains<Location> for AssetFeesFilter {
    fn contains(location: &Location) -> bool {
        location.parent_count() > 0
            && location.first_interior() != PolkadotXcmLocation::get().first_interior()
    }
}

// implement your own business logic for who can add/edit/remove/resume supported assets
pub type AddSupportedAssetOrigin = EnsureRoot<AccountId>;
pub type EditSupportedAssetOrigin = EnsureRoot<AccountId>;
pub type RemoveSupportedAssetOrigin = EnsureRoot<AccountId>;
pub type ResumeSupportedAssetOrigin = EnsureRoot<AccountId>;

impl pallet_xcm_weight_trader::Config for Runtime {
    type AccountIdToLocation = AccountIdToLocation;
    type AddSupportedAssetOrigin = AddSupportedAssetOrigin;
    type AssetLocationFilter = AssetFeesFilter;
    type AssetTransactor = AssetTransactors;
    type Balance = Balance;
    type EditSupportedAssetOrigin = EditSupportedAssetOrigin;
    type NativeLocation = SelfReserve;
    #[cfg(feature = "runtime-benchmarks")]
    type NotFilteredLocation = RelayLocation;
    type PauseSupportedAssetOrigin = EditSupportedAssetOrigin;
    type RemoveSupportedAssetOrigin = RemoveSupportedAssetOrigin;
    type ResumeSupportedAssetOrigin = ResumeSupportedAssetOrigin;
    type RuntimeEvent = RuntimeEvent;
    // TODO: update this when we update benchmarks
    type WeightInfo = weights::pallet_xcm_weight_trader::WeightInfo<Runtime>;
    type WeightToFee = <Runtime as pallet_transaction_payment::Config>::WeightToFee;
    type XcmFeesAccount = TreasuryAccount;
}
