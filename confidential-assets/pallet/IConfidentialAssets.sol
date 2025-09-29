// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

interface IConfidentialAssets {
    // Mirrors: set_operator(holder=msg.sender)
    function setOperator(
        uint128 asset,
        address operator,
        uint256 until
    ) external;

    // Mirrors: confidential_transfer(from=msg.sender)
    function confidentialTransfer(
        uint128 asset,
        address to,
        bytes calldata amount
    ) external;

    // Mirrors: confidential_mint (Root origin on-chain)
    function confidentialMint(
        uint128 asset,
        address to,
        bytes calldata amount
    ) external;

    // Mirrors: confidential_burn (Root origin on-chain)
    function confidentialBurn(
        uint128 asset,
        address from,
        bytes calldata amount
    ) external;

    // Mirrors: request_decryption
    function requestDecryption(bytes calldata encrypted) external;

    // Mirrors: decrypt
    function decrypt(
        uint128 request,
        uint128 amount,
        bytes calldata proof
    ) external;
}
