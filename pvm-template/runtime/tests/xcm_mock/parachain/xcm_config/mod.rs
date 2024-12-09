pub mod asset_transactor;
pub mod barrier;
pub mod constants;
pub mod location_converter;
pub mod origin_converter;
pub mod weigher;

use frame_support::traits::{Everything, Nothing};
use xcm_builder::{EnsureDecodableXcm, FixedRateOfFungible, FrameTransactionalProcessor};

use crate::xcm_mock::parachain::{MsgQueue, PolkadotXcm, RuntimeCall};

// Generated from `decl_test_network!`
pub type XcmRouter = EnsureDecodableXcm<crate::xcm_mock::ParachainXcmRouter<MsgQueue>>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type Aliasers = Nothing;
    type AssetClaims = ();
    type AssetExchanger = ();
    type AssetLocker = PolkadotXcm;
    type AssetTransactor = asset_transactor::AssetTransactor;
    type AssetTrap = ();
    type Barrier = barrier::Barrier;
    type CallDispatcher = RuntimeCall;
    type FeeManager = ();
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type HrmpNewChannelOpenRequestHandler = ();
    type IsReserve = ();
    type IsTeleporter = ();
    type MaxAssetsIntoHolding = constants::MaxAssetsIntoHolding;
    type MessageExporter = ();
    type OriginConverter = origin_converter::OriginConverter;
    type PalletInstancesInfo = ();
    type ResponseHandler = ();
    type RuntimeCall = RuntimeCall;
    type SafeCallFilter = Everything;
    type SubscriptionService = ();
    type Trader = FixedRateOfFungible<constants::KsmPerSecondPerByte, ()>;
    type TransactionalProcessor = FrameTransactionalProcessor;
    type UniversalAliases = Nothing;
    type UniversalLocation = constants::UniversalLocation;
    type Weigher = weigher::Weigher;
    type XcmRecorder = PolkadotXcm;
    type XcmSender = XcmRouter;
}
