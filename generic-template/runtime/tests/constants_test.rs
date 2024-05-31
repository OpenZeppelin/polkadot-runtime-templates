mod constant_tests {
    use parachain_template_runtime::constants::currency::*;

    #[test]
    fn test_constants() {
        assert_eq!(MICROCENTS, 1_000_000);

        assert_eq!(MILLICENTS, 1_000_000_000);

        assert_eq!(CENTS, 1_000 * MILLICENTS);

        assert_eq!(DOLLARS, 100 * CENTS);

        assert_eq!(EXISTENTIAL_DEPOSIT, MILLICENTS);

        // Ensure deposit function behavior remains constant
        assert_eq!(deposit(2, 3), 2 * 15 * CENTS + 3 * 6 * CENTS);
    }
}

mod runtime_tests {
    use frame_support::{pallet_prelude::Weight, traits::TypedGet, PalletId};
    use parachain_template_runtime::{
        configs::*,
        constants::{currency::*, *},
        *,
    };
    use sp_runtime::create_runtime_str;
    use sp_version::RuntimeVersion;
    use xcm::latest::prelude::BodyId;

    #[test]
    fn check_runtime_api_version() {
        assert_eq!(
            VERSION,
            RuntimeVersion {
                spec_name: create_runtime_str!("template-parachain"),
                impl_name: create_runtime_str!("template-parachain"),
                authoring_version: 1,
                spec_version: 1,
                impl_version: 0,
                apis: parachain_template_runtime::apis::RUNTIME_API_VERSIONS,
                transaction_version: 1,
                state_version: 1,
            }
        );
    }

    #[test]
    fn weight_to_fee_constants() {
        assert_eq!(P_FACTOR, 10);

        assert_eq!(Q_FACTOR, 100);

        assert_eq!(POLY_DEGREE, 1);
    }

    #[test]
    fn frame_system_constants() {
        #[cfg(not(feature = "async-backing"))]
        assert_eq!(
            MAXIMUM_BLOCK_WEIGHT,
            Weight::from_parts(
                frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
                cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64
            )
        );

        #[cfg(feature = "async-backing")]
        assert_eq!(
            MAXIMUM_BLOCK_WEIGHT,
            Weight::from_parts(
                frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2),
                cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64
            )
        );

        assert_eq!(AVERAGE_ON_INITIALIZE_RATIO, Perbill::from_percent(5));

        assert_eq!(NORMAL_DISPATCH_RATIO, Perbill::from_percent(75));
        #[cfg(not(feature = "async-backing"))]
        assert_eq!(UNINCLUDED_SEGMENT_CAPACITY, 1);
        #[cfg(feature = "async-backing")]
        assert_eq!(UNINCLUDED_SEGMENT_CAPACITY, 3);

        assert_eq!(BLOCK_PROCESSING_VELOCITY, 1);

        assert_eq!(RELAY_CHAIN_SLOT_DURATION_MILLIS, 6000);

        #[cfg(not(feature = "async-backing"))]
        assert_eq!(MILLISECS_PER_BLOCK, 12000);
        #[cfg(feature = "async-backing")]
        assert_eq!(MILLISECS_PER_BLOCK, 6000);

        assert_eq!(SLOT_DURATION, MILLISECS_PER_BLOCK);

        assert_eq!(MINUTES, 60_000 / (MILLISECS_PER_BLOCK as BlockNumber));

        assert_eq!(HOURS, MINUTES * 60);

        assert_eq!(DAYS, HOURS * 24);

        assert_eq!(MAX_BLOCK_LENGTH, 5 * 1024 * 1024);

        assert_eq!(SS58Prefix::get(), 42);

        assert_eq!(<Runtime as frame_system::Config>::MaxConsumers::get(), 16);
    }

    #[test]
    fn proxy_constants() {
        assert_eq!(MaxProxies::get(), 32);

        assert_eq!(MaxPending::get(), 32);

        assert_eq!(ProxyDepositBase::get(), deposit(1, 40));

        assert_eq!(AnnouncementDepositBase::get(), deposit(1, 48));

        assert_eq!(ProxyDepositFactor::get(), deposit(0, 33));

        assert_eq!(AnnouncementDepositFactor::get(), deposit(0, 66));
    }

    #[test]
    fn balances_constants() {
        assert_eq!(MaxFreezes::get(), 0);

        assert_eq!(MaxLocks::get(), 50);

        assert_eq!(MaxReserves::get(), 50);
    }

    #[test]
    fn assets_constants() {
        assert_eq!(AssetDeposit::get(), 10 * CENTS);

        assert_eq!(AssetAccountDeposit::get(), deposit(1, 16));

        assert_eq!(ApprovalDeposit::get(), EXISTENTIAL_DEPOSIT);

        assert_eq!(StringLimit::get(), 50);

        assert_eq!(MetadataDepositBase::get(), deposit(1, 68));

        assert_eq!(MetadataDepositPerByte::get(), deposit(0, 1));

        assert_eq!(RemoveItemsLimit::get(), 1000);
    }

    #[test]
    fn transaction_payment_constants() {
        assert_eq!(TransactionByteFee::get(), 10 * MICROCENTS);

        assert_eq!(OperationalFeeMultiplier::get(), 5);
    }

    #[test]
    fn cumulus_pallet_parachain_system_constants() {
        assert_eq!(ReservedXcmpWeight::get(), MAXIMUM_BLOCK_WEIGHT.saturating_div(4));

        assert_eq!(ReservedDmpWeight::get(), MAXIMUM_BLOCK_WEIGHT.saturating_div(4));
    }

    #[test]
    fn message_queue_constants() {
        assert_eq!(HeapSize::get(), 64 * 1024);
        assert_eq!(MaxStale::get(), 8);
    }

    #[test]
    fn cumulus_pallet_xcmp_queue_constants() {
        assert_eq!(MaxInboundSuspended::get(), 1000);
    }

    #[test]
    fn multisig_constants() {
        assert_eq!(DepositBase::get(), deposit(1, 88));

        assert_eq!(DepositFactor::get(), deposit(0, 32));

        assert_eq!(MaxSignatories::get(), 100);
    }

    #[test]
    fn session_constants() {
        assert_eq!(Period::get(), 6 * HOURS);

        assert_eq!(Offset::get(), 0);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn aura_constants() {
        #[cfg(not(feature = "async-backing"))]
        assert!(!AllowMultipleBlocksPerSlot::get());
        #[cfg(feature = "async-backing")]
        assert!(AllowMultipleBlocksPerSlot::get());

        assert_eq!(MaxAuthorities::get(), 100_000);
    }

    #[test]
    fn collator_selection_constants() {
        let pallet_id_to_string = |id: PalletId| -> String {
            core::str::from_utf8(&id.0).unwrap_or_default().to_string()
        };

        assert_eq!(pallet_id_to_string(PotId::get()), pallet_id_to_string(PalletId(*b"PotStake")));

        assert_eq!(SessionLength::get(), 6 * HOURS);

        assert_eq!(StakingAdminBodyId::get(), BodyId::Defense);

        assert_eq!(MaxCandidates::get(), 100);

        assert_eq!(MaxInvulnerables::get(), 20);

        assert_eq!(MinEligibleCollators::get(), 4);
    }
}

mod xcm_tests {
    use frame_support::weights::Weight;
    use parachain_template_runtime::configs::xcm_config::*;

    #[test]
    fn xcm_executor_constants() {
        assert_eq!(UnitWeightCost::get(), Weight::from_parts(1_000_000_000, 64 * 1024));
        assert_eq!(MaxInstructions::get(), 100);
        assert_eq!(MaxAssetsIntoHolding::get(), 64);
    }

    #[test]
    fn pallet_xcm_constants() {
        assert_eq!(MaxLockers::get(), 8);
        assert_eq!(MaxRemoteLockConsumers::get(), 0);
        assert_eq!(<parachain_template_runtime::Runtime as pallet_xcm::Config>::VERSION_DISCOVERY_QUEUE_SIZE, 100);
    }
}
