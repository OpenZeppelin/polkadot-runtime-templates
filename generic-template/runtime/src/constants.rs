pub mod currency {
    use crate::Balance;

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
