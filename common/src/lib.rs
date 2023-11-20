pub use currency::*;

mod currency {
    use polkadot_core_primitives::Balance;
    // Unit = the base number of indivisible units for balances
    pub const UNIT: Balance = 1_000_000_000_000;
    pub const MILLIUNIT: Balance = 1_000_000_000;
    pub const MICROUNIT: Balance = 1_000_000;

    /// The existential deposit. Set to 1/10 of the Connected Relay Chain.
    pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        (items as Balance * 20 * UNIT + bytes as Balance * 100 * MILLIUNIT) / 100
    }
}
