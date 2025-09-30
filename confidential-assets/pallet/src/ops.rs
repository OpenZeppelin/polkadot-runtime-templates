//! Types and traits to access the FHE host interface inside the pallet

// TODO: move to associated types for the pallet trait Config
pub type Balance = u128;
pub(crate) type AssetId = u128;
// 128 kiB
pub type Cipher = sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<131_072>>;
pub type RequestId = u64;

pub trait FheOps {
    fn add(a: Cipher, b: Cipher) -> Cipher;
    fn sub(x: Cipher, d: Cipher) -> Cipher;
    fn select(c: Cipher, x: Cipher, y: Cipher) -> Cipher;
    fn try_increase(a: Cipher, b: Cipher) -> (Cipher, Cipher);
    fn try_decrease(x: Cipher, d: Cipher) -> (Cipher, Cipher);
    fn as_zero() -> Cipher;
    fn is_initialized(x: Cipher) -> bool;
    fn allow_this(x: Cipher);
    fn allow_to<T: frame_system::Config>(x: Cipher, who: &T::AccountId);
    // Get from FHEVM coprocessor?
    // fn request_decryption(x: Cipher);
    // fn check_signatures(request: RequestId, amount: Cipher, proof: Cipher) -> bool;
}

#[cfg(test)]
// Default implementation for mock runtimes. Do NOT use in production.
impl FheOps for () {
    fn add(_a: Cipher, _b: Cipher) -> Cipher {
        Default::default()
    }

    fn sub(_x: Cipher, _d: Cipher) -> Cipher {
        Default::default()
    }

    fn select(_c: Cipher, _x: Cipher, _y: Cipher) -> Cipher {
        Default::default()
    }

    fn try_increase(_a: Cipher, _b: Cipher) -> (Cipher, Cipher) {
        Default::default()
    }

    fn try_decrease(_x: Cipher, _d: Cipher) -> (Cipher, Cipher) {
        Default::default()
    }

    fn as_zero() -> Cipher {
        Default::default()
    }

    fn is_initialized(_x: Cipher) -> bool {
        true
    }

    fn allow_this(_x: Cipher) {}

    fn allow_to<T: frame_system::Config>(_x: Cipher, _who: &T::AccountId) {}

    // Get from FHEVM coprocessor?
    // fn request_decryption(_x: Cipher) {}

    // fn check_signatures(_request: RequestId, _amount: Cipher, _proof: Cipher) -> bool {
    //     true
    // }
}
