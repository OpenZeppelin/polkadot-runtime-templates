//! Hook to expose amount disclosure directly to match
use crate::{Config, Event, Pallet};
use oz_fhe_primitives::oracle::{OracleFulfillHook, Purpose};

pub struct OnFheFulfillHook;

impl<T: Config> OracleFulfillHook<T> for OnFheFulfillHook {
    fn on_fulfill(id: u64, _who: &T::AccountId, purpose: Purpose, _payload: &[u8], result: &[u8]) {
        match purpose {
            Purpose::AmountDisclosure => {
                // payload: original ciphertext bytes
                // result: plaintext
                if result.len() == 16 {
                    let mut le = [0u8; 16];
                    le.copy_from_slice(result);
                    let amount = u128::from_le_bytes(le);
                    Pallet::<T>::deposit_event(Event::AmountDisclosed { id, amount });
                }
            }
            // Custom purposes do not concern this pallet. Examples of custom purposes include:
            // - private voting tabulation
            // - private auction execution
            // - private order-matching
            Purpose::Custom(_) => {}
        }
    }
}
