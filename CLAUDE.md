# mutr - Solidity Mutation Testing Runner

## Overview

A Rust CLI tool that runs mutation testing for Solidity projects using Foundry. Uses Gambit for mutation generation with an abstraction layer for future generator support.

## Roadmap

### Core Mutation Testing (Complete)

- [x] Project setup (flake.nix, Cargo.toml)
- [x] Gambit library integration
- [x] Test runner with temp workspace per mutant
- [x] Report mutation score + surviving mutants + which test killed
- [x] Wire up CLI to run full mutation testing flow

### Nix Package with Crane (Complete)

- [x] Add crane input to flake.nix
- [x] Build mutr with craneLib.buildPackage
- [x] Wrap binary to include forge in PATH
- [x] Export packages.default and apps.default
- [x] Usage: `nix run github:sveitser/mutr -- run --project .`

### Foundry.toml Configuration (Complete)

- [x] Parse foundry.toml for project settings
- [x] Auto-detect project root from file paths
- [x] Pass optimizer, evm_version, remappings to gambit
- [x] Resolve remappings via `forge remappings` when not in foundry.toml
- [x] Add --skip-validate flag (workaround for via_ir projects)
- [x] Progress output during mutation generation and testing
- [x] Note: via_ir not supported by gambit upstream
- [x] Note: gambit expects solc binary path, version strings from foundry.toml are ignored with warning

### Parallel Execution & CI Sharding (Complete)

- [x] Run multiple mutants concurrently with rayon
- [x] Configurable `--workers N` flag (default 1)
- [x] `generate` subcommand: produce manifest directory with mutant files
- [x] `test` subcommand: read manifest, apply `--partition slice:M/N`, run subset, output partial results
- [x] `report` subcommand: merge partial result files, print summary, `--fail-under` threshold
- [x] Manifest format: JSON manifest + mutant files with relative paths
- [x] Partition: round-robin assignment by mutant ID
- [x] GitHub Actions CI/CD (lint, test, coverage, mutation test, release)
- [x] `report --format markdown` for CI step summaries

### Target Configuration (Complete)

- [x] `mutr.toml` config file with `[[target]]` sections
- [x] Per-target `files`, `contracts`, `functions`, `forge_args`
- [x] Glob pattern support in target files
- [x] Conflict detection: error when both mutr.toml and CLI files/forge_args
- [x] Per-mutant `forge_args` through generate/manifest/test pipeline
- [x] Contract and function filters passed to gambit
- [x] `--config` flag to override mutr.toml path
- [x] Backward compatible: old manifests without forge_args deserialize fine

### Incremental Testing

- [ ] Cache test results by mutant hash
- [ ] Only re-test changed mutants

### Coverage Filtering

- [ ] Parse forge coverage output
- [ ] Only mutate lines covered by tests

## Development Environment

Use direnv with nix-direnv to automatically load the dev environment.

Use the rust-dev agent for Rust implementation tasks.

### Common Commands

- `just fmt` - Format all code (cargo fmt + prettier on tracked files)
- `just check` - Format, compile check, and lint
- `just test` - Run tests with nextest
- `just cov` - Run coverage and show summary
- `just cov-check` - Check coverage meets thresholds
- `just cov-html` - Open HTML coverage report
- `just cov-uncovered` - Show summary with uncovered lines listed per file
- `just cov-text` - Show annotated source with hit counts per line
- `just cov-regions` - Show uncovered regions from JSON coverage data
- `just cov-functions` - Show uncovered functions from JSON coverage data
- `just example` - Run mutr on the simple fixture
- `just clean` - Remove generated output

### Commits

Use semantic commit messages: `type: description`

- Present tense ("add" not "added")
- Bullet points in body for multiple changes
- feat: new feature
- fix: bug fix
- docs: documentation
- refactor: code restructuring
- test: adding tests
- chore: maintenance

### Pre-commit Hooks (via git-hooks.nix)

- rustfmt
- clippy (with -D warnings, runs on rust + toml)
- cargo nextest run (runs on rust + toml)
- cargo-llvm-cov (99% line, 97% region, 97% function coverage via `just cov-check`; skipped on Darwin)
- typos (spell checking)
- cargo-lock (sync Cargo.lock with Cargo.toml)
- nixpkgs-fmt (nix formatting)

### Tools in devShell

- Rust stable (via oxalica)
- cargo-nextest
- cargo-llvm-cov (Linux only)
- Foundry (from nixpkgs)
- just
- solc 0.8.30 (via EspressoSystems/nix-solc-bin)
- typos
- nixpkgs-fmt

## Architecture

```
mutr
├── src/
│   ├── main.rs           # Thin entry point, delegates to cli::run
│   ├── lib.rs            # Library root, test utilities
│   ├── cli.rs            # CLI logic: clap structs, subcommands, orchestration
│   ├── config.rs         # foundry.toml + mutr.toml parsing, project root detection, remapping resolution
│   ├── generator/
│   │   ├── mod.rs        # Generator trait, Mutant type, FileTarget
│   │   └── gambit.rs     # Gambit implementation with contract/function filters
│   ├── manifest.rs       # Manifest read/write for CI sharding
│   ├── partition.rs      # Round-robin partition for CI sharding
│   ├── runner.rs         # Test runner (forge test)
│   └── report.rs         # Results reporting, merge partial results, --format markdown
├── .github/
│   ├── actions/
│   │   ├── install-solc/ # Composite action: install solc binary
│   │   └── install-mutr/ # Composite action: install mutr from releases
│   └── workflows/
│       ├── lint.yml      # fmt, clippy, typos, prettier
│       ├── test.yml      # nextest, coverage
│       ├── mutation-test.yml  # Sharded mutation testing (self-test)
│       ├── release.yml   # Native multi-arch builds + GitHub release
│       └── example-mutation-test.yml  # Example sharded workflow for users
├── Cargo.toml
├── justfile              # Common dev commands
└── CLAUDE.md             # Roadmap
```

## Dependencies

### Runtime

- clap (derive): CLI parsing
- gambit (git v1.0.6): Mutation generation library
- glob: File discovery
- rayon: Parallel mutant execution
- serde + serde_json: Config and report serialization
- tempfile: Temp directory management
- thiserror: Typed errors for testable error paths
- toml: foundry.toml + mutr.toml parsing
- anyhow: Top-level error handling (main.rs only)

### External Tools (must be in PATH)

- forge: Test runner and remapping resolution (via subprocess)

### Dev Dependencies

- assert_cmd: CLI testing
- assert_fs: Filesystem fixtures
- predicates: Assertion helpers
- pretty_assertions: Better test output with assert_matches!

## Design Decisions

- **Temp workspace per mutant**: Cleaner isolation, enables parallelism
- **Generator trait**: Abstract mutation source for future flexibility
- **fail-fast**: Stop test run on first failure (mutant killed)
- **Gambit as library**: Use gambit crate directly via `run_mutate()` API
- **Forge as subprocess**: Use `forge test --json` CLI instead of library because:
  - Forge library internals (`MultiContractRunner`) are not designed for external use
  - Complex setup (solc config, project paths, compilation) poorly documented
  - CLI provides stable, versioned interface
  - JSON output gives same test result data we need
- **Forge for remappings**: Use `forge remappings --root` to resolve auto-detected remappings from libs directories
- **thiserror for internal errors**: Typed errors for testable code paths
- **anyhow at boundaries**: Only use anyhow in main.rs for unhandled errors
- **rayon for parallelism**: Local thread pool per run, not global, to support configurable worker count
- **ID-based partitioning**: Round-robin by mutant ID (contiguous 1-based from gambit) for deterministic shard assignment
- **Manifest with relative paths**: Stored relative to manifest dir, resolved on read for portability across CI runners
- **Per-mutant forge_args**: Each mutant carries its own forge_args from the target config, enabling different test filters per contract
- **mutr.toml mutually exclusive with CLI files/forge_args**: Prevents ambiguous configuration; global flags (workers, mutations, etc.) always from CLI
