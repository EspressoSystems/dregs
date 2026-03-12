# Run tests
test *args:
    cargo nextest run {{args}}

# Run coverage and show summary
cov *args:
    cargo llvm-cov {{args}}

# Run coverage and open HTML report
cov-html *args:
    cargo llvm-cov --html --open {{args}}

# Check coverage meets 100% threshold
cov-check *args:
    cargo llvm-cov --fail-under-lines 100 {{args}}

# Format, check, and lint
check *args:
    cargo fmt {{args}}
    cargo check {{args}}
    cargo clippy --all-targets -- -D warnings {{args}}

# Clean generated output (gambit_out, lcov.info)
clean:
    rm -rfv gambit_out lcov.info tests/fixtures/*/gambit_out tests/fixtures/*/out tests/fixtures/*/cache

# Run mutr on the simple fixture
example:
    cargo run -- run --project tests/fixtures/simple

fmt:
    cargo fmt
    git ls-files | xargs prettier -w --ignore-unknown
