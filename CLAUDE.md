# mutr - Solidity Mutation Testing Runner

## Overview
A Rust CLI tool that runs mutation testing for Solidity projects using Foundry. Uses Gambit for mutation generation with an abstraction layer for future generator support.

## Status: In Progress

## Roadmap

### MVP (Current)
- [x] Project setup (flake.nix, Cargo.toml)
- [x] Gambit library integration
- [x] Test runner with temp workspace per mutant
- [ ] Report mutation score + surviving mutants + which test killed
- [ ] Wire up CLI to run full mutation testing flow

### v0.2 - Parallel Execution
- [ ] Run multiple mutants concurrently
- [ ] Configurable worker count

### v0.3 - Incremental Testing
- [ ] Cache test results by mutant hash
- [ ] Only re-test changed mutants

### v0.4 - Coverage Filtering
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

### Nix Flake Inputs
```nix
inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
inputs.rust-overlay.url = "github:oxalica/rust-overlay";
inputs.git-hooks.url = "github:cachix/git-hooks.nix";
inputs.solc.url = "github:EspressoSystems/nix-solc-bin";
```

### Pre-commit Hooks (via git-hooks.nix)
- rustfmt
- clippy (with -D warnings, runs on rust + toml)
- cargo nextest run (runs on rust + toml)
- cargo-llvm-cov (100% line coverage required, runs on rust + toml; skipped on Darwin)
- typos (spell checking)
- cargo-lock (sync Cargo.lock with Cargo.toml)
- nixpkgs-fmt (nix formatting)

### Tools in devShell
- Rust stable (via oxalica)
- cargo-nextest
- cargo-llvm-cov (Linux only)
- Foundry (from nixpkgs)
- solc 0.8.30 (via EspressoSystems/nix-solc-bin)
- typos
- nixpkgs-fmt

### Gambit Integration
Use as Rust git dependency (has lib.rs with `run_mutate()` API):
```toml
[dependencies]
gambit = { git = "https://github.com/Certora/gambit", tag = "v1.0.6" }
```

### Foundry Integration
Run `forge test` via subprocess (simpler than using forge library internals):
- Copy project to temp directory
- Apply mutant file
- Run `forge test --json` and parse output
- Extract which test killed the mutant from failure output

## Testing

### Unit Tests
- Pure functions tested with standard `#[test]`
- Mock file system operations where needed

### CLI Integration Tests (assert_cmd + assert_fs)
```rust
// tests/cli.rs
use assert_cmd::Command;

#[test]
fn test_run_simple_project() {
    let mut cmd = Command::cargo_bin("mutr").unwrap();
    cmd.arg("run")
       .arg("--project")
       .arg("tests/fixtures/simple")
       .assert()
       .success();
}
```

### Test Fixtures (tests/fixtures/)
Embedded minimal Foundry projects (solc 0.8.30):
```
tests/fixtures/
├── simple/           # Basic Counter.sol + test
│   ├── src/Counter.sol
│   ├── test/Counter.t.sol
│   └── foundry.toml
└── multi-file/       # Multiple contracts (later)
```

### Test Harness Helper
```rust
// tests/common/mod.rs
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
```

## Architecture

```
mutr
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library root
│   ├── generator/
│   │   ├── mod.rs        # Generator trait
│   │   └── gambit.rs     # Gambit implementation
│   ├── runner.rs         # Test runner (forge test)
│   └── report.rs         # Results reporting
├── Cargo.toml
└── CLAUDE.md             # Roadmap
```

## Core Interfaces

### Generator Trait
```rust
trait MutationGenerator {
    fn generate(&self, config: &GeneratorConfig) -> Result<Vec<Mutant>>;
}

struct Mutant {
    id: u32,
    source_path: PathBuf,      // Original file
    mutant_path: PathBuf,      // Mutated file
    operator: String,          // e.g., "binary-op-mutation"
    original: String,          // Original code
    replacement: String,       // Mutated code
    line: u32,
}
```

### Runner
```rust
struct TestResult {
    mutant_id: u32,
    killed: bool,
    killed_by: Option<String>,  // e.g., "CounterTest::test_increment"
    duration: Duration,
}

fn run_mutant(mutant: &Mutant, project_root: &Path) -> Result<TestResult>
```

## MVP Implementation

### Step 1: Project Setup
- Initialize Cargo project with clap for CLI
- Dependencies: clap, serde, serde_json, tempfile, thiserror, anyhow

### Step 2: Gambit Integration
- Use `gambit` crate directly via `gambit::run_mutate()`
- Implement `GambitGenerator` that calls library API
- Returns `Vec<Mutant>` directly (no JSON parsing needed)

### Step 3: Test Runner
- Run `forge test` via subprocess (simpler than library)
- For each mutant:
  1. Create temp workspace (copy project, exclude build artifacts)
  2. Apply mutant file in temp workspace
  3. Run `forge test --json` with timeout
  4. Parse output to capture which test killed the mutant
  5. Clean up temp workspace (automatic via tempfile)
- Temp workspace approach enables future parallelism

### Step 4: Reporting
- Calculate mutation score: `killed / total`
- Print summary table with surviving mutants
- Output JSON report for CI integration

## CLI Design

```
mutr run [OPTIONS] [FILES]

Arguments:
  [FILES]  Solidity files to mutate (default: src/**/*.sol)

Options:
  -p, --project <PATH>       Project root (default: .)
  -o, --output <PATH>        Output report path (JSON)
  --fail-under <SCORE>       Fail if mutation score below threshold (0.0-1.0)
  --solc <PATH>              Path to solc binary
  --mutations <OPS>          Comma-separated mutation operators (default: all)
  --timeout <SECS>           Test timeout per mutant in seconds (default: 60)
```

### Mutation Operators (from gambit)
- binary-op-mutation
- require-mutation
- assignment-mutation
- delete-expression-mutation
- if-cond-mutation
- swap-arguments-operator-mutation
- unary-operator-mutation
- elim-delegate-mutation
- function-call-mutation
- swap-arguments-function-mutation

## Workflow

1. `mutr run` - single command does everything:
   - Calls gambit library to generate mutants -> ./gambit_out/
   - Tests each mutant with forge test
   - Reports mutation score + surviving mutants
2. User inspects ./gambit_out/mutants/ for surviving mutant code

## Output Format (terminal)
```
[1/50] src/Counter.sol:12 binary-op-mutation: KILLED by CounterTest::test_increment
[2/50] src/Counter.sol:15 require-mutation: SURVIVED
...
Mutation score: 48/50 (96%)
Surviving mutants: 2 (see ./gambit_out/mutants/2/, ./gambit_out/mutants/7/)
```

## Temp Workspace Strategy
For MVP: full project copy to temp dir
Future optimization: symlink lib/, node_modules/, copy only src/

## Dependencies

### Runtime
- clap (derive): CLI parsing
- gambit (git v1.0.6): Mutation generation library
- serde + serde_json: Config and report serialization
- tempfile: Temp directory management
- thiserror: Typed errors for testable error paths
- anyhow: Top-level error handling (main.rs only)

### External Tools (must be in PATH)
- forge: Test runner (via subprocess)

### Dev Dependencies
- assert_cmd: CLI testing
- assert_fs: Filesystem fixtures
- predicates: Assertion helpers

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
- **thiserror for internal errors**: Typed errors for testable code paths
- **anyhow at boundaries**: Only use anyhow in main.rs for unhandled errors
