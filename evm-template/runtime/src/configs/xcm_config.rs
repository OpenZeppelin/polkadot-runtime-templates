use core::marker::PhantomData;

use frame_support::{
    parameter_types,
    traits::{ConstU32, Contains, ContainsPair, Everything, Nothing, PalletInfoAccess},
    weights::Weight,
};
use frame_system::EnsureRoot;
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use parity_scale_codec::{Decode, Encode};
use polkadot_parachain_primitives::primitives::{self, Sibling};
use scale_info::TypeInfo;
use sp_runtime::{traits::MaybeEquivalence, Vec};
use xcm::latest::prelude::{Assets as XcmAssets, *};
use xcm_builder::{
    AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom, Case,
    ConvertedConcreteId, DenyReserveTransferToRelayChain, DenyThenTry, EnsureXcmOrigin,
    FixedWeightBounds, FrameTransactionalProcessor, FungibleAdapter, FungiblesAdapter, HandleFee,
    IsChildSystemParachain, IsConcrete, NoChecking, ParentIsPreset, RelayChainAsNative,
    SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountKey20AsNative,
    SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId, UsingComponents,
    WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
use xcm_executor::{
    traits::{FeeReason, JustTry, TransactAsset},
    XcmExecutor,
};
use xcm_primitives::{
    AbsoluteAndRelativeReserve, AccountIdToLocation, AsAssetType, UtilityAvailableCalls,
    UtilityEncodeCall, XcmTransact,
};

use crate::{
    configs::{
        AssetType, ParachainSystem, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, WeightToFee,
        XcmpQueue,
    },
    types::{AccountId, AssetId, Balance, XcmFeesToAccount},
    weights, AllPalletsWithSystem, AssetManager, Assets, Balances, ParachainInfo, PolkadotXcm,
    Treasury,
};

parameter_types! {
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

type Reserves = (
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
    /// Please, keep these two configs (`IsReserve` and `IsTeleporter`) mutually exclusive.
    /// The IsReserve type must be set to specify which <MultiAsset, MultiLocation> pair we trust to deposit reserve assets on our chain. We can also use the unit type () to block ReserveAssetDeposited instructions.
    /// The IsTeleporter type must be set to specify which <MultiAsset, MultiLocation> pair we trust to teleport assets to our chain. We can also use the unit type () to block ReceiveTeleportedAssets instruction.
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
    type Trader = (
        UsingComponents<WeightToFee, BalancesPalletLocation, AccountId, Balances, ()>,
        xcm_primitives::FirstAssetTrader<AssetType, AssetManager, XcmFeesToAccount>,
    );
    type TransactionalProcessor = FrameTransactionalProcessor;
    type UniversalAliases = Nothing;
    // Teleporting is disabled.
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type XcmRecorder = PolkadotXcm;
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
    type MaxRemoteLockConsumers = MaxRemoteLockConsumers;
    type RemoteLockConsumerIdentifier = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type SovereignAccountOf = LocationToAccountId;
    type TrustedLockers = ();
    type UniversalLocation = UniversalLocation;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
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
    pub SelfReserve: Location = Location {
        parents:0,
        interior: [
            PalletInstance(<Balances as PalletInfoAccess>::index() as u8)
        ].into()
    };
}

impl pallet_xcm_transactor::Config for Runtime {
    type AccountIdToLocation = AccountIdToLocation<AccountId>;
    type AssetTransactor = AssetTransactors;
    type Balance = Balance;
    type BaseXcmWeight = BaseXcmWeight;
    // we do not currently differentiate between SelfReserve and ForeignAssets
    type CurrencyId = AssetId;
    type CurrencyIdToLocation = AccountIdToLocation<AccountId>;
    type DerivativeAddressRegistrationOrigin = EnsureRoot<AccountId>;
    type HrmpManipulatorOrigin = EnsureRoot<AccountId>;
    type HrmpOpenOrigin = EnsureRoot<AccountId>;
    type MaxHrmpFee = xcm_builder::Case<MaxHrmpRelayFee>;
    type ReserveProvider = AbsoluteAndRelativeReserve<SelfLocationAbsolute>;
    type RuntimeEvent = RuntimeEvent;
    type SelfLocation = SelfLocation;
    type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId>;
    type Transactor = Transactors;
    type UniversalLocation = UniversalLocation;
    type Weigher = XcmWeigher;
    // TODO set
    type WeightInfo = ();
    type XcmSender = XcmRouter;
}

// Our currencyId. We distinguish for now between SelfReserve, and Others, defined by their Id.
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum CurrencyId {
    SelfReserve,
    ForeignAsset(AssetId),
}

// How to convert from CurrencyId to Location
pub struct CurrencyIdToLocation<AssetXConverter>(sp_std::marker::PhantomData<AssetXConverter>);
impl<AssetXConverter> sp_runtime::traits::Convert<CurrencyId, Option<Location>>
    for CurrencyIdToLocation<AssetXConverter>
where
    AssetXConverter: MaybeEquivalence<Location, AssetId>,
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

/// Xcm Weigher shared between multiple Xcm-related configs.
pub type XcmWeigher =
    WeightInfoBounds<weights::XcmWeight<Runtime, RuntimeCall>, RuntimeCall, MaxInstructions>;
