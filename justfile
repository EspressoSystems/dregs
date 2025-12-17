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
