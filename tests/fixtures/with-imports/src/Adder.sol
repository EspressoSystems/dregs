// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Math} from "@mylib/Math.sol";

contract Adder {
    uint256 public total;

    function add(uint256 a, uint256 b) public returns (uint256) {
        uint256 result = Math.add(a, b);
        require(result > 0, "result must be positive");
        total = total + result;
        return result;
    }
}
