frame_benchmarking::define_benchmarks!(
    [frame_system, SystemBench::<Runtime>]
    [pallet_assets, Assets]
    [pallet_balances, Balances]
    [pallet_session, SessionBench::<Runtime>]
    [pallet_timestamp, Timestamp]
    [pallet_message_queue, MessageQueue]
    [pallet_sudo, Sudo]
    [pallet_collator_selection, CollatorSelection]
    [cumulus_pallet_xcmp_queue, XcmpQueue]
    [pallet_scheduler, Scheduler]
    [pallet_preimage, Preimage]
    [pallet_proxy, Proxy]
    [cumulus_pallet_parachain_system, ParachainSystem]
    [pallet_multisig, Multisig]
    [pallet_utility, Utility]
    [pallet_treasury, Treasury]
    [pallet_evm, EVM]
    [pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
    [pallet_asset_manager, AssetManager]
    [pallet_conviction_voting, ConvictionVoting]
    [pallet_whitelist, Whitelist]
    [pallet_referenda, Referenda]
    [pallet_xcm_transactor, XcmTransactor]
    [pallet_xcm_weight_trader, XcmWeightTrader]
);

use cumulus_primitives_core::{ChannelStatus, GetChannelInfo};
use frame_support::traits::{
    tokens::{Pay, PaymentStatus},
    Get,
};
use sp_std::marker::PhantomData;

use crate::ParachainSystem;

/// Trait for setting up any prerequisites for successful execution of benchmarks.
pub trait EnsureSuccessful {
    fn ensure_successful();
}

/// Implementation of the [`EnsureSuccessful`] trait which opens an HRMP channel between
/// the Collectives and a parachain with a given ID.
pub struct OpenHrmpChannel<I>(PhantomData<I>);
impl<I: Get<u32>> EnsureSuccessful for OpenHrmpChannel<I> {
    fn ensure_successful() {
        if let ChannelStatus::Closed = ParachainSystem::get_channel_status(I::get().into()) {
            ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(I::get().into())
        }
    }
}

/// Type that wraps a type implementing the [`Pay`] trait to decorate its
/// [`Pay::ensure_successful`] function with a provided implementation of the
/// [`EnsureSuccessful`] trait.
pub struct PayWithEnsure<O, E>(PhantomData<(O, E)>);
impl<O, E> Pay for PayWithEnsure<O, E>
where
    O: Pay,
    E: EnsureSuccessful,
{
    type AssetKind = O::AssetKind;
    type Balance = O::Balance;
    type Beneficiary = O::Beneficiary;
    type Error = O::Error;
    type Id = O::Id;

    fn pay(
        who: &Self::Beneficiary,
        asset_kind: Self::AssetKind,
        amount: Self::Balance,
    ) -> Result<Self::Id, Self::Error> {
        O::pay(who, asset_kind, amount)
    }

    fn check_payment(id: Self::Id) -> PaymentStatus {
        O::check_payment(id)
    }

    fn ensure_successful(
        who: &Self::Beneficiary,
        asset_kind: Self::AssetKind,
        amount: Self::Balance,
    ) {
        E::ensure_successful();
        O::ensure_successful(who, asset_kind, amount)
    }

    fn ensure_concluded(id: Self::Id) {
        O::ensure_concluded(id)
    }
}
