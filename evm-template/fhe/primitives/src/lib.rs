#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod fhe;
#[cfg(feature = "std")]
use fhe::*;
use sp_runtime_interface::runtime_interface;
use sp_runtime::Vec;

#[runtime_interface]
pub trait FheHost {
    fn add(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_add_impl(a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::add body is never executed in Wasm; host call is injected");
        }
    }

    fn sub(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_sub_impl(a, b);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::sub body is never executed in Wasm; host call is injected");
        }
    }

    fn select(cond_ebool: Vec<u8>, x: Vec<u8>, y: Vec<u8>) -> Vec<u8> {
        #[cfg(feature = "std")]
        {
            return fhe_select_impl(cond_ebool, x, y);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::select body is never executed in Wasm; host call is injected");
        }
    }

    // SafeMath-like helpers:

    fn try_increase(x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        #[cfg(feature = "std")]
        {
            return fhe_try_increase_impl(x, delta);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::try_increase body is never executed in Wasm; host call is injected"
            );
        }
    }

    fn try_decrease(x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        #[cfg(feature = "std")]
        {
            return fhe_try_decrease_impl(x, delta);
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

    fn is_initialized(x: Vec<u8>) -> bool {
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

    fn allow_this(x: Vec<u8>) {
        #[cfg(feature = "std")]
        {
            acl_allow_this_impl(x);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!(
                "FheHost::allow_this body is never executed in Wasm; host call is injected"
            );
        }
    }

    fn allow_to(x: Vec<u8>, who: Vec<u8>) {
        #[cfg(feature = "std")]
        {
            acl_allow_to_impl(x, who);
        }
        #[cfg(not(feature = "std"))]
        {
            unreachable!("FheHost::allow_to body is never executed in Wasm; host call is injected");
        }
    }
}
