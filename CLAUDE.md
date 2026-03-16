# dregs - Solidity Mutation Testing Runner

A Rust CLI tool that runs mutation testing for Solidity projects using Foundry. Uses Gambit for mutation generation with an abstraction layer for future generator support.

## Features

Subcommands:

- `run` - all-in-one generate + test + report
- `generate` - write mutant manifest
- `test` - run tests from manifest
- `report` - merge partial results
- `inspect` - view/test mutants from a manifest

Key flags (see `dregs <subcommand> --help` for all):

- `--workers N` - parallel execution (run)
- `--diff-base REF` - only changed lines from git (run, generate)
- `--diff-file PATH` - only changed lines from file, `-` for stdin (run, generate)
- `--fail-under SCORE` - score threshold (run, report)
- `--partition slice:M/N` - CI sharding (test)
- `--mutations OPS` - comma-separated mutation operators (run, generate)
- `--timeout SECS` - per mutant (run)
- `--skip-validate` - via_ir workaround (run, test)
- `--config PATH` - dregs.toml path (run, generate)
- `--ids IDS` - comma-separated mutant IDs (inspect)
- `--results PATH` - results file to filter survived mutants (inspect)
- `--test` - run forge test for selected mutants (inspect)
- `-- <forge_args>` - forwarded to forge test (run, test, inspect)

Configuration:

- foundry.toml: auto-detected, reads optimizer/evm_version/remappings; remappings resolved via `forge remappings` if absent; solc version strings ignored (requires binary path)
- dregs.toml: `[[target]]` with files (glob), contracts, functions, forge_args; mutually exclusive with CLI files/forge_args
- Source comments: `dregs:ignore` (single line), `dregs:ignore-start`/`dregs:ignore-end` (block) to suppress mutations

Diff filtering (`--diff-base` or `--diff-file`):

- Pre-generation file filter + post-generation line filter
- `--diff-base`: merge-base semantics (`REF...HEAD`)
- `--diff-file`: read unified diff from file (`-` for stdin); mutually exclusive with `--diff-base`
- Clean exit (100% score) when no changes affect targets

CI sharding:

- generate -> test (partition slice:M/N, round-robin by mutant ID) -> report (merge + `--format markdown`)
- Manifest uses relative paths for portability; stores `ignored_ids` for ignored mutants

Ignore comments:

- `dregs:ignore` on a line excludes mutations on that line
- `dregs:ignore-start`/`dregs:ignore-end` for block exclusion
- Ignored mutants excluded from score (not in numerator or denominator)
- Errors on unclosed blocks, unmatched ends, nested starts

Execution:

- Fail-fast on first test failure (mutant killed)
- Baseline test validation before mutation testing
- Temp workspace per mutant for isolation

## Future Work

### Incremental Testing

- Cache test results by mutant hash
- Only re-test changed mutants

### Coverage Filtering

- Parse forge coverage output
- Only mutate lines covered by tests

## Development

- Use direnv with flake.nix to automatically load the dev environment.
- All (non-crate) dependencies installed via flake.nix

- `just fmt` - Format (cargo fmt + prettier)
- `just check` - Format, compile, lint
- `just test` - Tests (nextest)
- `just example` - Run dregs on simple fixture
- `just clean` - Remove generated output
- `just cov` - Coverage summary
- `just cov-check` - Check coverage thresholds (used by pre-commit)
- `just cov-html` - HTML coverage report
- `just cov-uncovered` - Uncovered lines per file
- `just cov-text` - Annotated source with hit counts
- `just cov-regions` - Uncovered regions from JSON data
- `just cov-functions` - Uncovered functions from JSON data

Coverage commands use `cargo-llvm-cov` (Linux only).

### Commits

Semantic commits, present tense, bullet points. Types: feat, fix, docs, refactor, test, chore.

### Pre-commit Hooks

rustfmt, clippy (-D warnings), nextest, cargo-llvm-cov (99% line/97% region/97% function; skipped on Darwin), typos, cargo-lock, nixpkgs-fmt

## Architecture

```
src/
  main.rs          - entry point, delegates to cli::run
  lib.rs           - library root, test utilities
  cli.rs           - clap structs, subcommands (run, generate, test, report, inspect), orchestration
  config.rs        - foundry.toml + dregs.toml parsing, project root, remappings
  diff.rs          - diff parsing (git/file/reader), mutant/target filtering by changed lines
  ignore.rs        - dregs:ignore comment parsing, mutant filtering by ignored lines
  generator/
    mod.rs         - Generator trait, Mutant type, FileTarget
    gambit.rs      - Gambit implementation with contract/function filters
  manifest.rs      - manifest read/write for CI sharding
  partition.rs     - round-robin partition for CI sharding
  runner.rs        - test runner (forge test)
  report.rs        - results reporting, merge partial results, markdown format
.github/
  actions/
    install-solc/  - composite action: install solc
    install-dregs/ - composite action: install dregs from releases
  workflows/
    lint.yml       - fmt, clippy, typos, prettier
    test.yml       - nextest, coverage
    mutation-test.yml      - sharded mutation testing (self-test)
    release.yml            - multi-arch builds + GitHub release
    example-mutation-test.yml - example sharded workflow for users
```

## Design Decisions

- **Binary crate, no public API**: only `Cli` and `run` are re-exported from lib.rs for main.rs; all modules are private. Don't mark items `pub` unless they're used from another module.
- **Generator trait**: abstract mutation source for future flexibility
- **Gambit as library**: use gambit crate directly via `run_mutate()` API
- **Forge as subprocess**: forge internals not designed for library use
- **thiserror for internal errors**: typed errors for testable code paths
- **anyhow at boundaries**: main.rs and cli.rs orchestration only; internal modules use thiserror
- **Per-mutant forge_args**: each mutant carries forge_args from target config
