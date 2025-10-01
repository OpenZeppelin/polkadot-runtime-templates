//! Runtime pallet-confidential-assets configuration

use oz_fhe_primitives::fhe_host;
use pallet_confidential_assets::{pallet::Config, Cipher, FheOps};
use parity_scale_codec::Encode;
use sp_runtime::BoundedVec;

use crate::{Runtime, RuntimeEvent};

pub struct FheOperator;

impl FheOps for FheOperator {
    #[inline]
    fn add(a: Cipher, b: Cipher) -> Cipher {
        BoundedVec::truncate_from(fhe_host::add(a.to_vec(), b.to_vec()))
    }

    #[inline]
    fn sub(a: Cipher, b: Cipher) -> Cipher {
        BoundedVec::truncate_from(fhe_host::sub(a.to_vec(), b.to_vec()))
    }

    #[inline]
    fn select(cond_ebool: Cipher, x: Cipher, y: Cipher) -> Cipher {
        BoundedVec::truncate_from(fhe_host::select(cond_ebool.to_vec(), x.to_vec(), y.to_vec()))
    }

    #[inline]
    fn try_increase(x: Cipher, delta: Cipher) -> (Cipher, Cipher) {
        let (a, b) = fhe_host::try_increase(x.to_vec(), delta.to_vec());
        (BoundedVec::truncate_from(a), BoundedVec::truncate_from(b))
    }

    #[inline]
    fn try_decrease(x: Cipher, delta: Cipher) -> (Cipher, Cipher) {
        let (a, b) = fhe_host::try_decrease(x.to_vec(), delta.to_vec());
        (BoundedVec::truncate_from(a), BoundedVec::truncate_from(b))
    }

    #[inline]
    fn as_zero() -> Cipher {
        BoundedVec::truncate_from(fhe_host::as_euint64_zero())
    }

    #[inline]
    fn is_initialized(x: Cipher) -> bool {
        fhe_host::is_initialized(x.clone().to_vec())
    }

    // Optional ACL hooks; these are no-ops in your current host impl
    #[inline]
    fn allow_this(x: Cipher) {
        fhe_host::allow_this(x.to_vec())
    }

    #[inline]
    fn allow_to<T: frame_system::Config>(x: Cipher, who_scale: &T::AccountId) {
        fhe_host::allow_to(x.to_vec(), who_scale.encode())
    }
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeFhe = FheOperator;
    type WeightInfo = ();
}
