//! Host-side helpers (only compiled with `std`)
//! Keep PURE/DETERMINISTIC (no IO/time/random).

#[cfg(feature = "std")]
use bincode::{deserialize, serialize};
#[cfg(feature = "std")]
use tfhe::prelude::*;
#[cfg(feature = "std")]
use tfhe::{FheBool, FheUint128}; // operators, overflowing_{add,sub}, if_then_else, etc. (docs.rs)

/// Utilities to (de)serialize ciphertexts.
/// You can swap the integer type in exactly one place if you need a different width.
#[cfg(feature = "std")]
mod ser {
    use super::*;
    pub fn de_u128(ct: Vec<u8>) -> FheUint128 {
        deserialize::<FheUint128>(&ct).expect("invalid FheUint128 ciphertext bytes")
    }
    pub fn ser_u128(ct: &FheUint128) -> Vec<u8> {
        serialize(ct).expect("serialize FheUint128 failed")
    }
    pub fn de_bool(ct: Vec<u8>) -> FheBool {
        deserialize::<FheBool>(&ct).expect("invalid FheBool ciphertext bytes")
    }
    pub fn ser_bool(ct: &FheBool) -> Vec<u8> {
        serialize(ct).expect("serialize FheBool failed")
    }
}

#[cfg(feature = "std")]
pub(crate) fn fhe_add_impl(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
    // Modular addition on encrypted integers (wraps on overflow).
    // This uses the high-level operators exposed by tfhe-rs. (docs.rs shows add/+=)
    // https://docs.rs/tfhe/1.3.3/tfhe/type.FheUint4.html (trait list applies to all FheUint)
    let a = ser::de_u128(a);
    let b = ser::de_u128(b);
    let sum = &a + &b; // homomorphic add
    ser::ser_u128(&sum)
}

#[cfg(feature = "std")]
pub(crate) fn fhe_sub_impl(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
    // Modular subtraction on encrypted integers (wraps on underflow).
    let a = ser::de_u128(a);
    let b = ser::de_u128(b);
    let diff = &a - &b; // homomorphic sub
    ser::ser_u128(&diff)
}

#[cfg(feature = "std")]
pub(crate) fn fhe_select_impl(cond: Vec<u8>, x: Vec<u8>, y: Vec<u8>) -> Vec<u8> {
    // Encrypted ternary select: cond ? x : y
    // Use FheBool::if_then_else(...) from the public API.
    // Docs: encrypted_condition.if_then_else(encrypted_if, encrypted_else)
    // https://docs.rs/tfhe/1.3.3/tfhe/struct.FheBool.html
    let cond = ser::de_bool(cond);
    let x = ser::de_u128(x);
    let y = ser::de_u128(y);
    let out = cond.if_then_else(&x, &y);
    ser::ser_u128(&out)
}

#[cfg(feature = "std")]
pub(crate) fn fhe_try_increase_impl(x: Vec<u8>, d: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    // Checked add with overflow flag.
    // tfhe-rs exposes overflowing_add returning (sum, FheBool overflow_flag).
    // https://docs.rs/tfhe/1.3.3/tfhe/type.FheUint4.html  (method visible in trait set)
    let x = ser::de_u128(x);
    let d = ser::de_u128(d);
    let (sum, overflow) = x.overflowing_add(&d); // overflow == true if wrapped
    let ok = !overflow; // success flag: true iff no overflow
    (ser::ser_bool(&ok), ser::ser_u128(&sum))
}

#[cfg(feature = "std")]
pub(crate) fn fhe_try_decrease_impl(x: Vec<u8>, d: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    // Checked sub with underflow flag.
    // overflowing_sub returns (diff, FheBool underflow_flag).
    // https://docs.rs/tfhe/1.3.3/tfhe/prelude/trait.OverflowingSub.html
    let x = ser::de_u128(x);
    let d = ser::de_u128(d);
    let (diff, underflow) = x.overflowing_sub(&d);
    let ok = !underflow; // true iff no borrow occurred
    (ser::ser_bool(&ok), ser::ser_u128(&diff))
}

#[cfg(feature = "std")]
pub(crate) fn fhe_zero_impl() -> Vec<u8> {
    // IMPORTANT: creating a *fresh* encryption of 0 requires the client key (randomness).
    // Server-only code must not encrypt. Keep 'zero' as a sentinel for "uninitialized".
    // Callers should treat an empty/marker buffer as 'not initialized' and avoid using it
    // in arithmetic until an actual ciphertext (e.g., x - x) is provided by the client.
    // The is_init check below enforces that.
    Vec::new()
}

#[cfg(feature = "std")]
pub(crate) fn fhe_is_init_impl(x: &Vec<u8>) -> bool {
    // Use "has bytes" as your init predicate. Do *not* try to deserialize here:
    // empty means "no ciphertext present".
    !x.is_empty()
}

// Access-control placeholders. Keep pure/deterministic.
// Record ACL in your own state if you wish, but don't do IO here.
#[cfg(feature = "std")]
pub(crate) fn acl_allow_this_impl(_x: Vec<u8>) {}

#[cfg(feature = "std")]
pub(crate) fn acl_allow_to_impl(_x: Vec<u8>, _who_scale: Vec<u8>) {}
