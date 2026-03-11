# mutr

> **WIP** - Mutation testing for Solidity projects using Foundry.

Generates mutants with [Gambit](https://github.com/Certora/gambit), runs
`forge test` against each, and reports which mutants survived.

## Install

```bash
nix run github:EspressoSystems/mutr.git -- run --project .
```

## Development

Uses nix with direnv to load the dev environment.

```bash
just build
just test
just cov
```

## Usage

### Simple run

```bash
mutr run --project ./my-foundry-project
```

```
Generating mutants...
Generated 12 mutants
[1/12] src/Counter.sol:10 binary-op-mutation -> KILLED by CounterTest::test_increment (1.2s)
[2/12] src/Counter.sol:15 require-mutation -> SURVIVED (0.8s)
     `require(true)` -> `require(false)`
...
Mutation score: 10/12 (83%)
Surviving mutants:
  [2] src/Counter.sol:15 require-mutation
     `require(true)` -> `require(false)`
```

### Parallel execution

```bash
mutr run --project . --workers 4
```

### Sharding (generate once, test in parallel jobs)

Generate a manifest:

```bash
mutr generate --project . --output ./mutants
```

Test partitions independently (e.g. in CI):

```bash
mutr test --manifest ./mutants/manifest.json --project . --partition slice:1/4 --output results-1.json
mutr test --manifest ./mutants/manifest.json --project . --partition slice:2/4 --output results-2.json
mutr test --manifest ./mutants/manifest.json --project . --partition slice:3/4 --output results-3.json
mutr test --manifest ./mutants/manifest.json --project . --partition slice:4/4 --output results-4.json
```

Merge results and report:

```bash
mutr report ./mutants/manifest.json results-*.json --fail-under 0.8
```

### Passing arguments to forge

Everything after `--` is forwarded to `forge test`:

```bash
mutr run --project . -- --match-contract CounterTest
mutr run --project . -- --match-test "test_increment|test_decrement"
```

```
Matched 2 tests:
  CounterTest::test_increment
  CounterTest::test_decrement
Running baseline tests...
Baseline tests passed (1.5s)
Generating mutants...
...
```
