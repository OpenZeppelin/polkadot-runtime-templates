// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

interface IConfidentialAssets {
    function setOperator(
        uint128 asset,
        address operator,
        uint256 until
    ) external;
    function confidentialTransfer(
        uint128 asset,
        address to,
        bytes calldata amount
    ) external;

    function confidentialMint(
        uint128 asset,
        address to,
        bytes calldata amount
    ) external;

    function confidentialBurn(
        uint128 asset,
        address from,
        bytes calldata amount
    ) external;

    function requestDecryption(bytes calldata encrypted) external;

    function decrypt(
        uint128 request,
        uint128 amount,
        bytes calldata proof
    ) external;
}
