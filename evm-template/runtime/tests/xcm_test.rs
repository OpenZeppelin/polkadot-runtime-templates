mod xcm_mock;
use frame_support::{
    assert_ok,
    traits::{ConstU32, PalletInfo, PalletInfoAccess},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
    BoundedVec,
};
// use pallet_xcm_transactor::{
// 	Currency, CurrencyPayment, HrmpInitParams, HrmpOperation, TransactWeights,
// };
use sp_runtime::traits::MaybeEquivalence;
use sp_std::boxed::Box;
use xcm::{
    latest::prelude::{
        AccountId32, AccountKey20, All, BuyExecution, ClearOrigin, DepositAsset, GeneralIndex,
        Junction, Junctions, Limited, Location, OriginKind, PalletInstance, Parachain,
        QueryResponse, Reanchorable, Response, WeightLimit, WithdrawAsset, Xcm,
    },
    VersionedLocation, WrapVersion,
};
use xcm_executor::traits::ConvertLocation;
use xcm_mock::*;
use xcm_primitives::{UtilityEncodeCall, DEFAULT_PROOF_SIZE};
use xcm_simulator::TestExt;
mod common;
use cumulus_primitives_core::relay_chain::HrmpChannelId;
// Send a relay asset (like DOT) to a parachain A
#[test]
fn receive_relay_asset_from_relay() {
    MockNet::reset();

    let source_location = parachain::AssetType::Xcm(xcm::v3::Location::parent());
    let source_id: parachain::AssetId = source_location.clone().into();
    let asset_metadata = parachain::AssetMetadata {
        name: b"RelayToken".to_vec(),
        symbol: b"Relay".to_vec(),
        decimals: 12,
    };
    // register relay asset in parachain A
    ParaA::execute_with(|| {
        assert_ok!(AssetManager::register_foreign_asset(
            parachain::RuntimeOrigin::root(),
            source_location.clone(),
            asset_metadata,
            1u128,
            true
        ));
        assert_ok!(AssetManager::set_asset_units_per_second(
            parachain::RuntimeOrigin::root(),
            source_location,
            0u128,
            0
        ));
    });

    // Actually send relay asset to parachain
    let dest: Location = AccountKey20 { network: None, key: PARAALICE }.into();
    Relay::execute_with(|| {
        assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
            relay_chain::RuntimeOrigin::signed(RELAYALICE),
            Box::new(Parachain(1).into()),
            Box::new(VersionedLocation::V4(dest).clone().into()),
            Box::new(([], 123).into()),
            0,
        ));
    });

    // Verify that parachain received the asset
    ParaA::execute_with(|| {
        // free execution, full amount received
        assert_eq!(Assets::balance(source_id, &PARAALICE.into()), 123);
    });
}
