//! Runtime pallet-confidential-assets configuration

use pallet_confidential_assets::{pallet::Config, FheOps};
use sp_fhe::fhe_host;
use sp_std::vec::Vec;

use crate::{Runtime, RuntimeEvent};

pub struct FheOperator;

impl FheOps for FheOperator {
    #[inline]
    fn add(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        fhe_host::add(a, b)
    }

    #[inline]
    fn sub(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
        fhe_host::sub(a, b)
    }

    #[inline]
    fn select(cond_ebool: Vec<u8>, x: Vec<u8>, y: Vec<u8>) -> Vec<u8> {
        fhe_host::select(cond_ebool, x, y)
    }

    #[inline]
    fn try_increase(x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        fhe_host::try_increase(x, delta)
    }

    #[inline]
    fn try_decrease(x: Vec<u8>, delta: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        fhe_host::try_decrease(x, delta)
    }

    #[inline]
    fn as_zero() -> Vec<u8> {
        fhe_host::as_euint64_zero()
    }

    #[inline]
    fn is_initialized(x: &Vec<u8>) -> bool {
        fhe_host::is_initialized(x.clone())
    }

    // Optional ACL hooks; these are no-ops in your current host impl
    #[inline]
    fn allow_this(x: Vec<u8>) {
        fhe_host::allow_this(x)
    }

    #[inline]
    fn allow_to(x: Vec<u8>, who_scale: Vec<u8>) {
        fhe_host::allow_to(x, who_scale)
    }
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeFhe = FheOperator;
    type WeightInfo = ();
}
