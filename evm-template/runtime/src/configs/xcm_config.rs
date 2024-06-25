use core::marker::PhantomData;

use frame_support::{
    parameter_types,
    traits::{ConstU32, Contains, Everything, Nothing, PalletInfoAccess},
    weights::Weight,
    PalletId,
};
use frame_system::EnsureRoot;
use pallet_xcm::XcmPassthrough;
use polkadot_parachain_primitives::primitives::{self, Sibling};
use sp_runtime::traits::AccountIdConversion;
use xcm::latest::prelude::{Assets as XcmAssets, *};
use xcm_builder::{
    deposit_or_burn_fee, AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom,
    AllowTopLevelPaidExecutionFrom, ConvertedConcreteId, DenyReserveTransferToRelayChain,
    DenyThenTry, EnsureXcmOrigin, FixedWeightBounds, FrameTransactionalProcessor, FungibleAdapter,
    FungiblesAdapter, HandleFee, IsChildSystemParachain, IsConcrete, NativeAsset, NoChecking,
    ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
    SignedAccountKey20AsNative, SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId,
    UsingComponents, WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
use xcm_executor::{
    traits::{FeeReason, JustTry, TransactAsset},
    XcmExecutor,
};
use xcm_primitives::AsAssetType;

use crate::{
    configs::{
        AssetType, ParachainSystem, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, WeightToFee,
        XcmpQueue,
    },
    types::{AccountId, AssetId, Balance, XcmFeesToAccount},
    AllPalletsWithSystem, AssetManager, Assets, Balances, ParachainInfo, PolkadotXcm,
};

parameter_types! {
    pub const RelayLocation: Location = Location::parent();
    pub const RelayNetwork: Option<NetworkId> = None;
    pub AssetsPalletLocation: Location =
        PalletInstance(<Assets as PalletInfoAccess>::index() as u8).into();
    pub BalancesPalletLocation: Location = PalletInstance(<Balances as PalletInfoAccess>::index() as u8).into();
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
    pub UniversalLocation: InteriorLocation = Parachain(ParachainInfo::parachain_id().into()).into();
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

// This is a workaround. We have added Treasury in the master and it will be added in the next release.
// We will collect fees on this pseudo treasury account until Treasury is rolled out.
// When treasury will be introduced, it will use the same account for fee collection so transition should be smooth.
pub struct TreasuryAccount;

impl Get<AccountId> for TreasuryAccount {
    fn get() -> AccountId {
        const ID: PalletId = PalletId(*b"py/trsry");
        AccountIdConversion::<AccountId>::into_account_truncating(&ID)
    }
}

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
    type FeeManager = XcmFeeManagerFromComponents<
        IsChildSystemParachain<primitives::Id>,
        XcmFeeToAccount<Self::AssetTransactor, AccountId, TreasuryAccount>,
    >;
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type HrmpNewChannelOpenRequestHandler = ();
    type IsReserve = NativeAsset;
    type IsTeleporter = ();
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type MessageExporter = ();
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type ResponseHandler = PolkadotXcm;
    type RuntimeCall = RuntimeCall;
    type SafeCallFilter = Everything;
    type SubscriptionService = PolkadotXcm;
    type Trader = (
        UsingComponents<WeightToFee, BalancesPalletLocation, AccountId, Balances, ()>,
        xcm_primitives::FirstAssetTrader<AssetType, AssetManager, XcmFeesToAccount>,
    );
    type TransactionalProcessor = FrameTransactionalProcessor;
    type UniversalAliases = Nothing;
    // Teleporting is disabled.
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type XcmSender = XcmRouter;
}

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
    type MaxRemoteLockConsumers = MaxLockers;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = LocationToAccountId;
    type TrustedLockers = ();
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type XcmExecuteFilter = Nothing;
    // ^ Disable dispatchable execute on the XCM pallet.
    // Needs to be `Everything` for local testing.
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmReserveTransferFilter = Nothing;
    type XcmRouter = XcmRouter;
    type XcmTeleportFilter = Everything;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

impl cumulus_pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}
