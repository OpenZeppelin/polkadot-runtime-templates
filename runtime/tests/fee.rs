// Integration transaction fee tests i.e. adjusts to block saturation
mod common;
use common::*;
use frame_support::pallet_prelude::*;
use parachain_template_runtime::{Runtime, RuntimeBlockWeights};
use polkadot_runtime_common::MinimumMultiplier;
use sp_runtime::{traits::Convert, Perquintill};

#[test]
fn multiplier_can_grow_from_zero() {
    let minimum_multiplier = MinimumMultiplier::get();
    let target = Perquintill::from_percent(25)
        * RuntimeBlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
    // if the min is too small, then this will not change, and we are doomed forever.
    // the weight is 1/100th bigger than target.
    run_with_system_weight(target * 101 / 100, || {
        let next = <Runtime as pallet_transaction_payment::Config>::FeeMultiplierUpdate::convert(
            minimum_multiplier,
        );
        assert!(next > minimum_multiplier, "{:?} !>= {:?}", next, minimum_multiplier);
    })
}
