//! Types and traits to access the FHE host interface inside the pallet

// TODO: move to associated types for the pallet trait Config
pub type Balance = u128;
pub type AssetId = u128;
pub type Cipher = [u8; 32];
pub type RequestId = u64;

// TODO: Remove all instances and usage of AssetId from here
// and the Runtime Interface
pub trait FheOps {
    fn add(asset: AssetId, a: Cipher, b: Cipher) -> Cipher;
    fn sub(asset: AssetId, x: Cipher, d: Cipher) -> Cipher;
    fn select(asset: AssetId, c: Cipher, x: Cipher, y: Cipher) -> Cipher;
    fn try_increase(asset: AssetId, a: Cipher, b: Cipher) -> (Cipher, Cipher);
    fn try_decrease(asset: AssetId, x: Cipher, d: Cipher) -> (Cipher, Cipher);
    fn as_zero() -> Cipher;
    fn is_initialized(asset: AssetId, x: Cipher) -> bool;
    fn allow_this(asset: AssetId, x: Cipher);
    fn allow_to<T: frame_system::Config>(asset: AssetId, x: Cipher, who: &T::AccountId);
    fn request_decryption(x: Cipher);
    fn check_signatures(request: RequestId, amount: Balance, proof: Cipher) -> bool;
}

#[cfg(test)]
// Default implementation for mock runtimes. Do NOT use in production.
impl FheOps for () {
    fn add(_asset: AssetId, _a: Cipher, _b: Cipher) -> Cipher {
        Default::default()
    }

    fn sub(_asset: AssetId, _x: Cipher, _d: Cipher) -> Cipher {
        Default::default()
    }

    fn select(_asset: AssetId, _c: Cipher, _x: Cipher, _y: Cipher) -> Cipher {
        Default::default()
    }

    fn try_increase(_asset: AssetId, _a: Cipher, _b: Cipher) -> (Cipher, Cipher) {
        Default::default()
    }

    fn try_decrease(_asset: AssetId, _x: Cipher, _d: Cipher) -> (Cipher, Cipher) {
        Default::default()
    }

    fn as_zero() -> Cipher {
        Default::default()
    }

    fn is_initialized(_asset: AssetId, _x: Cipher) -> bool {
        true
    }

    fn allow_this(_asset: AssetId, _x: Cipher) {}

    fn allow_to<T: frame_system::Config>(_asset: AssetId, _x: Cipher, _who: &T::AccountId) {}

    fn request_decryption(_x: Cipher) {}

    fn check_signatures(_request: RequestId, _amount: Balance, _proof: Cipher) -> bool {
        true
    }
}
