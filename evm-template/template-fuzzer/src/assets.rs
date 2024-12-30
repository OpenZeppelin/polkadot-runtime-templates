use evm_runtime_template::{AccountId, RuntimeCall};
use fp_account::AccountId20;
use parity_scale_codec::Compact;

use crate::ExtrinsicData;

pub fn generate_extrinsics_stream(data: &mut [u8]) -> ExtrinsicData {
    let mut res = vec![];
    let (mut ptr, mut call) = generate_call(data);
    while let Some(generated_call) = call {
        res.push(generated_call);
        (ptr, call) = generate_call(ptr);
    }
    res
}

fn generate_call(data: &[u8]) -> (&[u8], Option<(u8, u8, RuntimeCall)>) {
    // RuntimeCall
    if (data.len() < 3) {
        return (data, None);
    }
    let lapse = data[0];
    let origin = data[1];
    let call_id = data[2];
    let mut next_ptr = &data[3..];

    let call = match call_id % 7 {
        0 => {
            if next_ptr.len() < 52 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let beneficiary = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[36..52]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[52..];
            RuntimeCall::Assets (
                pallet_assets::Call::mint {
                    id,
                    beneficiary,
                    amount
                }
            )
        },
        1 => {
            if next_ptr.len() < 52 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let who = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[36..52]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[52..];
            RuntimeCall::Assets (
                pallet_assets::Call::burn {
                    id,
                    who,
                    amount
                }
            )
        },
        2 => {
            if next_ptr.len() < 52 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let target = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[36..52]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[52..];
            RuntimeCall::Assets (
                pallet_assets::Call::transfer {
                    id,
                    target,
                    amount
                }
            )
        },
        3 => {
            if next_ptr.len() < 52 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let target = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[36..52]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[52..];
            RuntimeCall::Assets (
                pallet_assets::Call::transfer_keep_alive {
                    id,
                    target,
                    amount
                }
            )
        },
        4 => {
            if next_ptr.len() < 52 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let delegate = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[36..52]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[52..];
            RuntimeCall::Assets (
                pallet_assets::Call::approve_transfer { id, delegate, amount }
            )
        },
        5 => {
            if next_ptr.len() < 72 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            let mut data_20: [u8; 20] = [0u8; 20];
            data_20.copy_from_slice(&next_ptr[16..36]);
            let owner = AccountId20::from(data_20);
            data_20.copy_from_slice(&next_ptr[36..56]);
            let destination = AccountId20::from(data_20);
            data.copy_from_slice(&next_ptr[56..72]);
            let amount = u128::from_le_bytes(data);
            next_ptr = &next_ptr[72..];
            RuntimeCall::Assets (
                pallet_assets::Call::transfer_approved { id, owner, destination, amount }
            )
        },
        6 => {
            if next_ptr.len() < 32 {
                return (next_ptr, None);
            }
            let mut data: [u8; 16] = [0u8; 16];
            data.copy_from_slice(&next_ptr[0..16]);
            let id = Compact(u128::from_le_bytes(data));
            data.copy_from_slice(&next_ptr[16..32]);
            let min_balance = u128::from_le_bytes(data);
            next_ptr = &next_ptr[32..];
            RuntimeCall::Assets (
                pallet_assets::Call::set_min_balance { id, min_balance }
            )
        }
        _ => unreachable!()
    };

    (&next_ptr, Some((lapse, origin, call)))
}
