pub mod xcm_mock;
use frame_support::{assert_ok, weights::Weight};
use parity_scale_codec::Encode;
use xcm::prelude::*;
use xcm_mock::*;
use xcm_simulator::TestExt;

// Helper function for forming buy execution message
#[allow(dead_code)]
fn buy_execution<C>(fees: impl Into<Asset>) -> Instruction<C> {
    BuyExecution { fees: fees.into(), weight_limit: Unlimited }
}

#[test]
fn remote_account_ids_work() {
    child_account_account_id(1, ALICE);
    sibling_account_account_id(1, ALICE);
    parent_account_account_id(ALICE);
}

#[test]
fn dmp() {
    MockNet::reset();

    let remark = parachain::RuntimeCall::System(
        frame_system::Call::<parachain::Runtime>::remark_with_event { remark: vec![1, 2, 3] },
    );
    Relay::execute_with(|| {
        assert_ok!(RelayChainPalletXcm::send_xcm(
            Here,
            Parachain(1),
            Xcm(vec![Transact {
                origin_kind: OriginKind::SovereignAccount,
                require_weight_at_most: Weight::from_parts(INITIAL_BALANCE as u64, 1024 * 1024),
                call: remark.encode().into(),
            }]),
        ));
    });

    ParaA::execute_with(|| {
        use parachain::{RuntimeEvent, System};
        assert!(System::events().iter().any(|r| matches!(
            r.event,
            RuntimeEvent::System(frame_system::Event::Remarked { .. })
        )));
    });
}
