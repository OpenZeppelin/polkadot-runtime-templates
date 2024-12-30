use evm_runtime_template::{Runtime, RuntimeCall};
use frame_support::traits::Get;
use parity_scale_codec::DecodeLimit;

use crate::ExtrinsicData;

pub fn generate_extrinsic_stream(mut data: &[u8]) -> ExtrinsicData {
    std::iter::from_fn(|| DecodeLimit::decode_with_depth_limit(64, &mut data).ok())
        .filter(|(_, _, x)| !matches!(x, RuntimeCall::System(_)))
        .collect()
}

pub fn recursive_call_filter(call: &RuntimeCall, origin: usize) -> bool {
    match call {
        //recursion
        RuntimeCall::Sudo(
            pallet_sudo::Call::sudo { call }
            | pallet_sudo::Call::sudo_unchecked_weight { call, weight: _ },
        ) if origin == 0 => recursive_call_filter(call, origin),
        RuntimeCall::Utility(
            pallet_utility::Call::with_weight { call, weight: _ }
            | pallet_utility::Call::dispatch_as { as_origin: _, call }
            | pallet_utility::Call::as_derivative { index: _, call },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Utility(
            pallet_utility::Call::force_batch { calls }
            | pallet_utility::Call::batch { calls }
            | pallet_utility::Call::batch_all { calls },
        ) => calls.iter().map(|call| recursive_call_filter(call, origin)).all(|e| e),
        RuntimeCall::Scheduler(
            pallet_scheduler::Call::schedule_named_after {
                id: _,
                after: _,
                maybe_periodic: _,
                priority: _,
                call,
            }
            | pallet_scheduler::Call::schedule { when: _, maybe_periodic: _, priority: _, call }
            | pallet_scheduler::Call::schedule_named {
                when: _,
                id: _,
                maybe_periodic: _,
                priority: _,
                call,
            }
            | pallet_scheduler::Call::schedule_after {
                after: _,
                maybe_periodic: _,
                priority: _,
                call,
            },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Multisig(
            pallet_multisig::Call::as_multi_threshold_1 { other_signatories: _, call }
            | pallet_multisig::Call::as_multi {
                threshold: _,
                other_signatories: _,
                maybe_timepoint: _,
                call,
                max_weight: _,
            },
        ) => recursive_call_filter(call, origin),
        RuntimeCall::Whitelist(
            pallet_whitelist::Call::dispatch_whitelisted_call_with_preimage { call },
        ) => recursive_call_filter(call, origin),

        // restrictions
        RuntimeCall::Sudo(_) if origin != 0 => false,
        RuntimeCall::System(
            frame_system::Call::set_code { .. } | frame_system::Call::kill_prefix { .. },
        ) => false,
        #[cfg(not(feature = "tanssi"))]
        RuntimeCall::CollatorSelection(
            pallet_collator_selection::Call::set_desired_candidates { max },
        ) =>
            *max < <<Runtime as pallet_collator_selection::Config>::MaxCandidates as Get<u32>>::get(
            ),
        RuntimeCall::Balances(pallet_balances::Call::force_adjust_total_issuance { .. }) => false,

        _ => true,
    }
}
