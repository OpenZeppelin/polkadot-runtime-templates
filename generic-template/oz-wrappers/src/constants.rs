/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
#[cfg(feature = "async-backing")]
pub const MILLISECS_PER_BLOCK: u64 = 6000;
#[cfg(not(feature = "async-backing"))]
pub const MILLISECS_PER_BLOCK: u64 = 12000;

// NOTE: Currently it is not possible to change the slot duration after the
// chain has started. Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

pub mod currency {
    use crate::types::Balance;

    pub const MICROCENTS: Balance = 1_000_000;
    pub const MILLICENTS: Balance = 1_000_000_000;
    pub const CENTS: Balance = 1_000 * MILLICENTS; // assume this is worth about a cent.
    pub const DOLLARS: Balance = 100 * CENTS;
    pub const GRAND: Balance = 1_000 * DOLLARS;

    pub const EXISTENTIAL_DEPOSIT: Balance = MILLICENTS;

    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        items as Balance * 15 * CENTS + (bytes as Balance) * 6 * CENTS
    }
}
