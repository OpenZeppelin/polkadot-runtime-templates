contract Storage {
    uint pos0;
    mapping(address => uint) pos1;

    constructor() {
        pos0 = 1234;
        pos1[msg.sender] = 5678;
    }

    // You can read from a state variable without sending a transaction.
    function get() public view returns (uint) {
        return pos0;
    }
}

