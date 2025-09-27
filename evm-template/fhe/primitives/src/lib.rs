#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime_interface::runtime_interface;

#[runtime_interface]
pub trait FheHost {
    fn add(asset: Vec<u8>, a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_add_impl(asset, a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::add body is never executed in Wasm; host call is injected");
        }
    }

    fn sub(asset: Vec<u8>, a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_sub_impl(asset, a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::sub body is never executed in Wasm; host call is injected");
        }
    }

    fn select(asset: Vec<u8>, cond_ebool: Vec<u8>, x: Vec<u8>, y: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_select_impl(asset, cond_ebool, x, y);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::select body is never executed in Wasm; host call is injected");
        }
    }

    // SafeMath-like helpers:

    fn try_increase(asset: Vec<u8>, x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        #[cfg(feature = "std")]
        {
            return fhe_try_increase_impl(asset, x, delta);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::try_increase body is never executed in Wasm; host call is injected"
            );
        }
    }

    fn try_decrease(asset: Vec<u8>, x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        #[cfg(feature = "std")]
        {
            return fhe_try_decrease_impl(asset, x, delta);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::try_decrease body is never executed in Wasm; host call is injected"
            );
        }
    }

    // Utility

    fn as_euint64_zero() -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_zero_impl();
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::as_euint64_zero body is never executed in Wasm; host call is injected"
            );
        }
    }

    fn is_initialized(_asset: Vec<u8>, x: Vec<u8>) -> bool {
        #[cfg(feature = "std")]
        {
            return fhe_is_init_impl(&x);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::is_initialized body is never executed in Wasm; host call is injected"
            );
        }
    }

    // Optional: permissions / key-switch hooks if you model ACLs on host

    fn allow_this(asset: Vec<u8>, x: Vec<u8>) {
        #[cfg(feature = "std")]
        {
            acl_allow_this_impl(asset, x);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::allow_this body is never executed in Wasm; host call is injected"
            );
        }
    }

    fn allow_to(asset: Vec<u8>, x: Vec<u8>, who: Vec<u8>) {
        #[cfg(feature = "std")]
        {
            acl_allow_to_impl(asset, x, who);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::allow_to body is never executed in Wasm; host call is injected");
        }
    }
}

//
// -------- Host-side helpers (only compiled with `std`) --------
//   Implement these with tfhe-rs + your key/domain routing.
//   Keep them PURE/DETERMINISTIC (no IO/time/random).
//

#[cfg(feature = "std")]
fn fhe_add_impl(_asset: Vec<u8>, a: Vec<u8>, _b: Vec<u8>) -> Vec<u8> {
    // TODO: decode domain from _asset, pick server key, run tfhe-rs add(a,_b)
    a // stub
}

#[cfg(feature = "std")]
fn fhe_sub_impl(_asset: Vec<u8>, a: Vec<u8>, _b: Vec<u8>) -> Vec<u8> {
    a
} // stub

#[cfg(feature = "std")]
fn fhe_select_impl(_asset: Vec<u8>, _cond: Vec<u8>, x: Vec<u8>, _y: Vec<u8>) -> Vec<u8> {
    x
} // stub

#[cfg(feature = "std")]
fn fhe_try_increase_impl(_asset: Vec<u8>, x: Vec<u8>, _d: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    (vec![1], x) // (ebool=true, new_cipher) stub
}

#[cfg(feature = "std")]
fn fhe_try_decrease_impl(_asset: Vec<u8>, x: Vec<u8>, _d: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    (vec![1], x) // stub
}

#[cfg(feature = "std")]
fn fhe_zero_impl() -> Vec<u8> {
    vec![0]
} // stub

#[cfg(feature = "std")]
fn fhe_is_init_impl(x: &Vec<u8>) -> bool {
    !x.is_empty()
} // stub

#[cfg(feature = "std")]
fn acl_allow_this_impl(_asset: Vec<u8>, _x: Vec<u8>) {
    // TODO: register/keyswitch scheduling as needed
}

#[cfg(feature = "std")]
fn acl_allow_to_impl(_asset: Vec<u8>, _x: Vec<u8>, _who_scale: Vec<u8>) {
    // TODO: decode who (AccountId32 via SCALE or raw H160 len==20), record ACL
}
