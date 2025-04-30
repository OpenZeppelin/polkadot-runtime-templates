//! Runtime XCM Tests

pub mod xcm_mock;

use frame_support::{assert_ok, weights::Weight};
use parity_scale_codec::Encode;
use xcm::prelude::*;
use xcm_mock::*;
use xcm_simulator::{mock_message_queue::ReceivedDmp, TestExt};

// Helper function for forming buy execution message
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
                fallback_max_weight: Some(Weight::from_parts(INITIAL_BALANCE as u64, 1024 * 1024)),
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

#[test]
fn ump() {
    MockNet::reset();

    let remark = relay_chain::RuntimeCall::System(
        frame_system::Call::<relay_chain::Runtime>::remark_with_event { remark: vec![1, 2, 3] },
    );
    ParaA::execute_with(|| {
        assert_ok!(ParachainPalletXcm::send_xcm(
            Here,
            Parent,
            Xcm(vec![Transact {
                origin_kind: OriginKind::SovereignAccount,
                fallback_max_weight: Some(Weight::from_parts(INITIAL_BALANCE as u64, 1024 * 1024)),
                call: remark.encode().into(),
            }]),
        ));
    });

    Relay::execute_with(|| {
        use relay_chain::{RuntimeEvent, System};
        assert!(System::events().iter().any(|r| matches!(
            r.event,
            RuntimeEvent::System(frame_system::Event::Remarked { .. })
        )));
    });
}

#[test]
fn xcmp() {
    MockNet::reset();

    let remark = parachain::RuntimeCall::System(
        frame_system::Call::<parachain::Runtime>::remark_with_event { remark: vec![1, 2, 3] },
    );
    ParaA::execute_with(|| {
        assert_ok!(ParachainPalletXcm::send_xcm(
            Here,
            (Parent, Parachain(2)),
            Xcm(vec![Transact {
                origin_kind: OriginKind::SovereignAccount,
                fallback_max_weight: Some(Weight::from_parts(INITIAL_BALANCE as u64, 1024 * 1024)),
                call: remark.encode().into(),
            }]),
        ));
    });

    ParaB::execute_with(|| {
        use parachain::{RuntimeEvent, System};
        assert!(System::events().iter().any(|r| matches!(
            r.event,
            RuntimeEvent::System(frame_system::Event::Remarked { .. })
        )));
    });
}

#[test]
fn remote_locking_and_unlocking() {
    MockNet::reset();

    let locked_amount = 100;

    ParaB::execute_with(|| {
        let message = Xcm(vec![LockAsset {
            asset: (Here, locked_amount).into(),
            unlocker: Parachain(1).into(),
        }]);
        assert_ok!(ParachainPalletXcm::send_xcm(Here, Parent, message.clone()));
    });

    Relay::execute_with(|| {
        use pallet_balances::{BalanceLock, Reasons};
        assert_eq!(
            relay_chain::Balances::locks(&child_account_id(2)),
            vec![BalanceLock { id: *b"py/xcmlk", amount: locked_amount, reasons: Reasons::All }]
        );
    });

    ParaA::execute_with(|| {
        assert_eq!(
            ReceivedDmp::<parachain::Runtime>::get(),
            vec![Xcm(vec![NoteUnlockable {
                owner: (Parent, Parachain(2)).into(),
                asset: (Parent, locked_amount).into()
            }])]
        );
    });

    ParaB::execute_with(|| {
        // Request unlocking part of the funds on the relay chain
        let message = Xcm(vec![RequestUnlock {
            asset: (Parent, locked_amount - 50).into(),
            locker: Parent.into(),
        }]);
        assert_ok!(ParachainPalletXcm::send_xcm(Here, (Parent, Parachain(1)), message));
    });

    Relay::execute_with(|| {
        use pallet_balances::{BalanceLock, Reasons};
        // Lock is reduced
        assert_eq!(
            relay_chain::Balances::locks(&child_account_id(2)),
            vec![BalanceLock {
                id: *b"py/xcmlk",
                amount: locked_amount - 50,
                reasons: Reasons::All
            }]
        );
    });
}

/// Scenario:
/// A parachain transfers funds on the relay chain to another parachain account.
///
/// Asserts that the parachain accounts are updated as expected.
#[test]
fn withdraw_and_deposit() {
    MockNet::reset();

    let send_amount = 10;

    ParaA::execute_with(|| {
        let message = Xcm(vec![
            WithdrawAsset((Here, send_amount).into()),
            buy_execution((Here, send_amount)),
            DepositAsset { assets: AllCounted(1).into(), beneficiary: Parachain(2).into() },
        ]);
        // Send withdraw and deposit
        assert_ok!(ParachainPalletXcm::send_xcm(Here, Parent, message.clone()));
    });

    Relay::execute_with(|| {
        assert_eq!(
            relay_chain::Balances::free_balance(child_account_id(1)),
            INITIAL_BALANCE - send_amount
        );
        assert_eq!(
            relay_chain::Balances::free_balance(child_account_id(2)),
            INITIAL_BALANCE + send_amount
        );
    });
}

/// Scenario:
/// A parachain wants to be notified that a transfer worked correctly.
/// It sends a `QueryHolding` after the deposit to get notified on success.
///
/// Asserts that the balances are updated correctly and the expected XCM is sent.
#[test]
fn query_holding() {
    MockNet::reset();

    let send_amount = 10;
    let query_id_set = 1234;

    // Send a message which fully succeeds on the relay chain
    ParaA::execute_with(|| {
        let message = Xcm(vec![
            WithdrawAsset((Here, send_amount).into()),
            buy_execution((Here, send_amount)),
            DepositAsset { assets: AllCounted(1).into(), beneficiary: Parachain(2).into() },
            ReportHolding {
                response_info: QueryResponseInfo {
                    destination: Parachain(1).into(),
                    query_id: query_id_set,
                    max_weight: Weight::from_parts(1_000_000_000, 1024 * 1024),
                },
                assets: All.into(),
            },
        ]);
        // Send withdraw and deposit with query holding
        assert_ok!(ParachainPalletXcm::send_xcm(Here, Parent, message.clone(),));
    });

    // Check that transfer was executed
    Relay::execute_with(|| {
        // Withdraw executed
        assert_eq!(
            relay_chain::Balances::free_balance(child_account_id(1)),
            INITIAL_BALANCE - send_amount
        );
        // Deposit executed
        assert_eq!(
            relay_chain::Balances::free_balance(child_account_id(2)),
            INITIAL_BALANCE + send_amount
        );
    });

    // Check that QueryResponse message was received
    ParaA::execute_with(|| {
        assert_eq!(
            ReceivedDmp::<parachain::Runtime>::get(),
            vec![Xcm(vec![QueryResponse {
                query_id: query_id_set,
                response: Response::Assets(Assets::new()),
                max_weight: Weight::from_parts(1_000_000_000, 1024 * 1024),
                querier: Some(Here.into()),
            }])],
        );
    });
}
