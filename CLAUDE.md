# mutr - Solidity Mutation Testing Runner

## Overview

A Rust CLI tool that runs mutation testing for Solidity projects using Foundry. Uses Gambit for mutation generation with an abstraction layer for future generator support.

## Status: v0.3 Complete

## Roadmap

### MVP (Complete)

- [x] Project setup (flake.nix, Cargo.toml)
- [x] Gambit library integration
- [x] Test runner with temp workspace per mutant
- [x] Report mutation score + surviving mutants + which test killed
- [x] Wire up CLI to run full mutation testing flow

### v0.2 - Nix Package with Crane (Complete)

- [x] Add crane input to flake.nix
- [x] Build mutr with craneLib.buildPackage
- [x] Wrap binary to include forge in PATH
- [x] Export packages.default and apps.default
- [x] Usage: `nix run github:sveitser/mutr -- run --project .`

### v0.3 - Foundry.toml Configuration (Complete)

- [x] Parse foundry.toml for project settings
- [x] Auto-detect project root from file paths
- [x] Pass optimizer, evm_version, remappings to gambit
- [x] Resolve remappings via `forge remappings` when not in foundry.toml
- [x] Add --skip-validate flag (workaround for via_ir projects)
- [x] Progress output during mutation generation and testing
- [x] Note: via_ir not supported by gambit upstream
- [x] Note: gambit expects solc binary path, version strings from foundry.toml are ignored with warning

### v0.4 - Parallel Execution

- [ ] Run multiple mutants concurrently
- [ ] Configurable worker count

### v0.5 - Incremental Testing

- [ ] Cache test results by mutant hash
- [ ] Only re-test changed mutants

### v0.6 - Coverage Filtering

- [ ] Parse forge coverage output
- [ ] Only mutate lines covered by tests

## Development Environment

Use direnv with nix-direnv to automatically load the dev environment.

Use the rust-dev agent for Rust implementation tasks.

### Commits

Use semantic commit messages: `type: description`

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
- cargo-llvm-cov (99% line coverage required, runs on rust + toml; skipped on Darwin)
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
│   ├── main.rs           # CLI entry point, progress output
│   ├── lib.rs            # Library root
│   ├── config.rs         # foundry.toml parsing, project root detection, remapping resolution
│   ├── generator/
│   │   ├── mod.rs        # Generator trait
│   │   └── gambit.rs     # Gambit implementation
│   ├── runner.rs         # Test runner (forge test)
│   └── report.rs         # Results reporting
├── Cargo.toml
├── justfile              # Common dev commands
└── CLAUDE.md             # Roadmap
```

## Dependencies

### Runtime

- clap (derive): CLI parsing
- gambit (git v1.0.6): Mutation generation library
- glob: File discovery
- serde + serde_json: Config and report serialization
- tempfile: Temp directory management
- thiserror: Typed errors for testable error paths
- toml: foundry.toml parsing
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
