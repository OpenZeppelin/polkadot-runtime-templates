contract Storage {
    uint pos0;

    constructor() {
        pos0 = 1234;
    }

    // You can read from a state variable without sending a transaction.
    function get() public view returns (uint) {
        return pos0;
    }
}

