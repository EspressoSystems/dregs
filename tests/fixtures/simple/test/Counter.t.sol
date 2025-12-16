// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Counter} from "../src/Counter.sol";

contract CounterTest {
    Counter public counter;

    function setUp() public {
        counter = new Counter();
    }

    function test_SetNumber() public {
        counter.setNumber(42);
        assert(counter.number() == 42);
    }

    function test_Increment() public {
        counter.setNumber(5);
        counter.increment();
        assert(counter.number() == 6);
    }

    function test_Decrement() public {
        counter.setNumber(5);
        counter.decrement();
        assert(counter.number() == 4);
    }

    function test_DecrementZeroReverts() public {
        try counter.decrement() {
            revert("Expected revert");
        } catch {}
    }
}
