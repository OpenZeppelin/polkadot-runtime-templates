#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use core::marker::PhantomData;
use core::result::Result;

use fp_evm::{ExitError, ExitSucceed, PrecompileHandle, PrecompileOutput, PrecompileResult};
use sp_core::{hashing, H256, U256};
use sp_io::storage;
use sp_std::vec::Vec;
use tfhe::integer::{RadixCiphertext, ServerKey};

// Public address
pub const FHE_ALU_U64: u64 = 0x1001;

const NS: &[u8] = b"fhe_alu";
const K_NONCE: &[u8] = b"fhe_alu/nonce";
const K_SK: &[u8] = b"fhe_alu/server_key";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CtHandle(pub H256);

#[inline]
fn key_for_handle(h: CtHandle) -> Vec<u8> {
    let mut k = NS.to_vec();
    k.extend_from_slice(h.0.as_bytes());
    k
}

#[inline]
fn read_nonce() -> u128 {
    storage::get(K_NONCE)
        .map(|v| {
            let mut b = [0u8; 16];
            b.copy_from_slice(&v[..16]);
            u128::from_le_bytes(b)
        })
        .unwrap_or(0)
}

#[inline]
fn write_nonce(n: u128) {
    storage::set(K_NONCE, &n.to_le_bytes());
}

#[inline]
fn bin_serialize<T: sp_core::serde::Serialize>(t: &T) -> Result<Vec<u8>, fp_evm::PrecompileFailure> {
    bincode::serde::encode_to_vec(t, bincode::config::standard())
        .map_err(|_| fail("bincode: encode"))
}

#[inline]
fn bin_deserialize<T: for<'de> sp_core::serde::Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, fp_evm::PrecompileFailure> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(v, _consumed)| v)
        .map_err(|_| fail("bincode: decode"))
}

// Ciphertext store (opaque TFHE blobs)
fn store_put_ct(ct: &RadixCiphertext) -> Result<CtHandle, fp_evm::PrecompileFailure> {
    let bytes = bin_serialize(ct)?;
    let nonce = read_nonce();

    // Domain-separated keccak: keccak( "CT" || nonce || payload )
    let dom = [0x43u8, 0x54]; // "CT"
    let mut nbuf = [0u8; 16];
    nbuf.copy_from_slice(&nonce.to_le_bytes());

    let hash = hashing::keccak_256(&[&dom[..], &nbuf[..], &bytes[..]].concat());
    let h = CtHandle(H256(hash));

    storage::set(&key_for_handle(h), &bytes);
    write_nonce(nonce.wrapping_add(1));
    Ok(h)
}

fn store_get_ct(h: CtHandle) -> Result<RadixCiphertext, fp_evm::PrecompileFailure> {
    let Some(bytes) = storage::get(&key_for_handle(h)) else {
        return Err(fail("FHE: unknown handle"));
    };
    bin_deserialize::<RadixCiphertext>(&bytes)
}

fn load_server_key() -> Result<ServerKey, fp_evm::PrecompileFailure> {
    let Some(bytes) = storage::get(K_SK) else {
        return Err(fail("FHE: missing SK"));
    };
    bin_deserialize::<ServerKey>(&bytes)
}

#[inline]
fn fail(msg: &'static str) -> fp_evm::PrecompileFailure {
    fp_evm::PrecompileFailure::Error { exit_status: ExitError::Other(msg.into()) }
}

#[inline]
fn ok(output: Vec<u8>) -> PrecompileOutput {
    PrecompileOutput { exit_status: ExitSucceed::Returned, output }
}

#[inline]
fn read_h256(input: &[u8], off: usize) -> Result<H256, fp_evm::PrecompileFailure> {
    input.get(off..off + 32).ok_or_else(|| fail("abi: oob")).map(|s| H256::from_slice(s))
}

#[inline]
fn read_u256(input: &[u8], off: usize) -> Result<U256, fp_evm::PrecompileFailure> {
    let slice = input.get(off..off + 32).ok_or_else(|| fail("abi: oob"))?;
    Ok(U256::from_big_endian(slice))
}

#[inline]
fn write_h256(h: H256) -> Vec<u8> {
    let mut out = vec![0u8; 32];
    out.copy_from_slice(h.as_bytes());
    out
}

#[inline]
fn u64_from_u256(x: U256) -> Result<u64, fp_evm::PrecompileFailure> {
    if x.bits() > 64 {
        Err(fail("k too large"))
    } else {
        Ok(x.low_u64())
    }
}

// FHE_ALU precompile
pub struct FheAlu<R>(PhantomData<R>);

impl<R> FheAlu<R>
where
    R: pallet_evm::Config + frame_system::Config,
{
    const SEL_ADD: [u8; 4] = [0x10, 0x00, 0x00, 0x01];
    const SEL_ADDPLAIN: [u8; 4] = [0x10, 0x00, 0x00, 0x02];
    #[cfg(feature = "fhe-dev")]
    const SEL_DEV_DUMP_CT: [u8; 4] = [0x10, 0xFF, 0xFF, 0x02];
    #[cfg(feature = "fhe-dev")]
    const SEL_DEV_SEED_CT: [u8; 4] = [0x10, 0xFF, 0xFF, 0x01];
    // dev selectors (behind `fhe-dev`)
    #[cfg(feature = "fhe-dev")]
    const SEL_DEV_SET_SK: [u8; 4] = [0x10, 0xFF, 0xFF, 0x00];
    const SEL_SUB: [u8; 4] = [0x10, 0x00, 0x00, 0x03];
    const SEL_SUBPLAIN: [u8; 4] = [0x10, 0x00, 0x00, 0x04];

    pub fn execute(handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        let input_vec = handle.input().to_vec();
        let input: &[u8] = &input_vec;
        if input.len() < 4 {
            return Some(Err(fail("bad selector")));
        }
        let sel = <[u8; 4]>::try_from(&input[0..4]).unwrap();

        // Dispatch
        let res = if sel == Self::SEL_ADD {
            Self::fhe_add(handle, input)
        } else if sel == Self::SEL_ADDPLAIN {
            Self::fhe_add_plain(handle, input)
        } else if sel == Self::SEL_SUB {
            Self::fhe_sub(handle, input)
        } else if sel == Self::SEL_SUBPLAIN {
            Self::fhe_sub_plain(handle, input)
        } else {
            #[cfg(feature = "fhe-dev")]
            {
                if sel == Self::SEL_DEV_SET_SK {
                    return Some(Self::dev_set_sk(handle, input));
                } else if sel == Self::SEL_DEV_SEED_CT {
                    return Some(Self::dev_seed_ct(handle, input));
                } else if sel == Self::SEL_DEV_DUMP_CT {
                    return Some(Self::dev_dump_ct(handle, input));
                }
            }
            return Some(Err(fail("unknown selector")));
        };

        Some(res)
    }

    // ---- add(bytes32 a, bytes32 b) -> bytes32
    fn fhe_add(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        // selector (0..4) | a(4..36) | b(36..68)
        let a = CtHandle(read_h256(input, 4)?);
        let b = CtHandle(read_h256(input, 36)?);

        let sk = load_server_key()?;
        let mut ca = store_get_ct(a)?;
        let mut cb = store_get_ct(b)?;
        let cc = sk.smart_add(&mut ca, &mut cb); // tfhe-rs expects &mut

        let out_h = store_put_ct(&cc)?;
        handle
            .record_cost(250_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(write_h256(out_h.0)))
    }

    // ---- addPlain(bytes32 a, uint256 k) -> bytes32
    fn fhe_add_plain(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        // selector | a(4..36) | k(36..68)
        let a = CtHandle(read_h256(input, 4)?);
        let k = u64_from_u256(read_u256(input, 36)?)?;

        let sk = load_server_key()?;
        let mut ca = store_get_ct(a)?;
        let cc = sk.smart_scalar_add(&mut ca, k);

        let out_h = store_put_ct(&cc)?;
        handle
            .record_cost(160_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(write_h256(out_h.0)))
    }

    // ---- sub(bytes32 a, bytes32 b) -> bytes32
    fn fhe_sub(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        // selector | a | b
        let a = CtHandle(read_h256(input, 4)?);
        let b = CtHandle(read_h256(input, 36)?);

        let sk = load_server_key()?;
        let mut ca = store_get_ct(a)?;
        let mut cb = store_get_ct(b)?;
        let cc = sk.smart_sub(&mut ca, &mut cb);

        let out_h = store_put_ct(&cc)?;
        handle
            .record_cost(250_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(write_h256(out_h.0)))
    }

    // ---- subPlain(bytes32 a, uint256 k) -> bytes32
    fn fhe_sub_plain(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        let a = CtHandle(read_h256(input, 4)?);
        let k = u64_from_u256(read_u256(input, 36)?)?;

        let sk = load_server_key()?;
        let mut ca = store_get_ct(a)?;
        let cc = sk.smart_scalar_sub(&mut ca, k);

        let out_h = store_put_ct(&cc)?;
        handle
            .record_cost(160_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(write_h256(out_h.0)))
    }

    // ---------------- DEV-ONLY selectors ----------------
    #[cfg(feature = "fhe-dev")]
    fn dev_set_sk(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        // selector | bytes sk (ABI dynamic): for dev we accept raw bytes32-aligned payload after selector
        // Simpler: treat everything after 4 bytes as the bincode blob (you can also ABI-decode if you prefer).
        let sk_bytes = &input[4..];
        // Validate
        let _sk: ServerKey = bin_deserialize(sk_bytes)?;
        storage::set(K_SK, sk_bytes);
        handle
            .record_cost(50_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(Vec::new()))
    }

    #[cfg(feature = "fhe-dev")]
    fn dev_seed_ct(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        let ct_bytes = &input[4..];
        let ct: RadixCiphertext = bin_deserialize(ct_bytes)?;
        let h = store_put_ct(&ct)?;
        handle
            .record_cost(80_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(write_h256(h.0)))
    }

    #[cfg(feature = "fhe-dev")]
    fn dev_dump_ct(handle: &mut impl PrecompileHandle, input: &[u8]) -> PrecompileResult {
        let h = CtHandle(read_h256(input, 4)?);
        let ct = store_get_ct(h)?;
        let bytes = bin_serialize(&ct)?;
        handle
            .record_cost(60_000)
            .map_err(|e| fp_evm::PrecompileFailure::Error { exit_status: e })?;
        Ok(ok(bytes))
    }
}
