// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

contract Counter {
    uint256 public number;

    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }

    function increment() public {
        number = number + 1;
    }

    function decrement() public {
        require(number > 0, "Cannot decrement below zero");
        number = number - 1;
    }
}
