// In your tests directory, create a new file, e.g., runtime/tests/constants_test.rs

use parachain_template_runtime::constants::currency::*;

#[test]
fn test_constants() {
    assert_eq!(MICROCENTS, 1_000_000);

    assert_eq!(MILLICENTS, 1_000_000_000);

    assert_eq!(CENTS, 1_000 * MILLICENTS);

    assert_eq!(DOLLARS, 100 * CENTS);

    assert_eq!(EXISTENTIAL_DEPOSIT, MILLICENTS);

    // Ensure deposit function behavior remains constant
    assert_eq!(deposit(2, 3), 2 * 15 * CENTS + 3 * 6 * CENTS);
}
