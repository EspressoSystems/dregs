# dregs

> **WIP** - Mutation testing for Solidity projects using Foundry.
>
> Binary crate only. No public Rust API.

Generates mutants with [Gambit](https://github.com/Certora/gambit), runs `forge test` against each, and reports which
mutants survived.

## Install

### From GitHub releases

Download a binary from the [releases page](https://github.com/EspressoSystems/dregs/releases).

### With cargo-binstall

```bash
cargo binstall --git https://github.com/EspressoSystems/dregs dregs
```

### In GitHub Actions

```yaml
- uses: taiki-e/install-action@v2
  with:
    tool: dregs
```

### From source

```bash
cargo install --git https://github.com/EspressoSystems/dregs
```

Requires `forge` and `solc` in PATH.

### With nix

```bash
nix run github:EspressoSystems/dregs -- run --project .
```

## Development

Uses nix with direnv to load the dev environment.

```bash
just build
just test
just cov
```

## Releasing

Releases use [cargo-release](https://github.com/crate-ci/cargo-release) and [git-cliff](https://git-cliff.org/).

```bash
cargo release patch --execute  # or minor, major
```

This bumps the version in `Cargo.toml`, generates the changelog, commits, tags, and pushes. The
[release workflow](.github/workflows/release.yml) triggers on the tag push and attaches build artifacts. Without
`--execute`, it runs as a dry run.

## Usage

### Simple run

```bash
dregs run --project ./my-foundry-project
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
dregs run --project . --workers 4
```

### Diff-based filtering

Only mutate lines changed since a git ref (useful for PR CI):

```bash
dregs run --project . --diff-base main
dregs run --project . --diff-base HEAD~1
```

Uses merge-base semantics (`git diff main...HEAD`), so on a PR branch this covers exactly the changes introduced by the
branch.

Alternatively, provide a pre-computed unified diff via file or stdin:

```bash
git diff main...HEAD -- '*.sol' > changes.diff
dregs run --project . --diff-file changes.diff
git diff main...HEAD -- '*.sol' | dregs run --project . --diff-file -
```

### Sharding (generate once, test in parallel jobs)

Generate a manifest:

```bash
dregs generate --project . --output ./mutants
```

Test partitions independently (e.g. in CI):

```bash
dregs test --manifest ./mutants/manifest.json --project . --partition slice:1/4 --output results-1.json
dregs test --manifest ./mutants/manifest.json --project . --partition slice:2/4 --output results-2.json
dregs test --manifest ./mutants/manifest.json --project . --partition slice:3/4 --output results-3.json
dregs test --manifest ./mutants/manifest.json --project . --partition slice:4/4 --output results-4.json
```

Merge results and report:

```bash
dregs report ./mutants/manifest.json results-*.json --fail-under 0.8
```

### Ignoring mutants

Add `dregs:ignore` comments in your Solidity source to suppress mutations on specific lines or blocks:

```solidity
function admin() public { // dregs:ignore
    owner = msg.sender;
}

// dregs:ignore-start
function legacyDeposit() public {
    // ...
}
// dregs:ignore-end
```

Ignored mutants are excluded from the score (not counted in numerator or denominator). The count is shown in the summary
when non-zero.

### Inspecting mutants

View mutant details from a manifest:

```bash
dregs inspect ./mutants/manifest.json
dregs inspect ./mutants/manifest.json --ids 2,5,9
dregs inspect ./mutants/manifest.json --results report.json
```

Optionally re-run tests for selected mutants:

```bash
dregs inspect ./mutants/manifest.json --ids 2 --test --project .
dregs inspect ./mutants/manifest.json --results report.json --test --project . -- --match-contract CounterTest
```

### CI

See [`.github/workflows/example-mutation-test.yml`](.github/workflows/example-mutation-test.yml) for a sharded GitHub
Actions workflow using release binaries.

See the [install-dregs action](.github/actions/install-dregs/action.yml) for installing from releases.

### Target configuration

Create a `dregs.toml` in your project root to pair contracts with their tests. See
[`tests/fixtures/simple/dregs.toml`](tests/fixtures/simple/dregs.toml) for a working example.

```toml
[[target]]
files = ["src/Token.sol"]
contracts = ["Token"]
forge_args = ["--match-contract", "TokenTest"]

[[target]]
files = ["src/Vault.sol"]
functions = ["deposit", "withdraw"]
forge_args = ["--match-contract", "VaultTest"]

[[target]]
files = ["src/Admin.sol"]
exclude_functions = ["pause", "unpause"]

[[target]]
files = ["src/utils/**/*.sol"]
```

Each target specifies:

- `files` (required) - Solidity files to mutate, supports glob patterns
- `contracts` (optional) - filter mutations to these contracts
- `functions` (optional) - filter mutations to these functions
- `exclude_functions` (optional) - exclude these functions from mutation (mutually exclusive with `functions`).
  Preferred over `functions` because new functions added to the contract will automatically be mutation-tested.
- `forge_args` (optional) - arguments passed to `forge test` for these mutants

When `dregs.toml` exists, CLI file arguments and `-- forge_args` are not allowed (mutually exclusive). Global flags like
`--workers`, `--mutations`, `--skip-validate`, and `--fail-under` are always from the CLI.

Use `--config` to specify a custom config path:

```bash
dregs run --project . --config path/to/custom.toml
```

### Passing arguments to forge

Everything after `--` is forwarded to `forge test`:

```bash
dregs run --project . -- --match-contract CounterTest
dregs run --project . -- --match-test "test_increment|test_decrement"
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
