# Run tests
test *args:
    cargo nextest run {{args}}

# Run coverage and show summary
cov *args:
    cargo llvm-cov {{args}}

# Run coverage and open HTML report
cov-html *args:
    cargo llvm-cov --html --open {{args}}

# Check coverage meets thresholds (99% lines, 97% regions, 97% functions)
cov-check *args:
    cargo llvm-cov --fail-under-lines 99 --fail-under-regions 97 --fail-under-functions 97 {{args}}

# Show summary with uncovered lines listed per file
cov-uncovered *args:
    cargo llvm-cov --show-missing-lines {{args}}

# Show annotated source with hit counts per line
cov-text *args:
    cargo llvm-cov --text {{args}}

# Show uncovered regions from JSON coverage data
cov-regions *args:
    #!/usr/bin/env python3
    import json, subprocess, sys
    out = subprocess.run(
        ["cargo", "llvm-cov", "--json"] + "{{args}}".split(),
        stdout=subprocess.PIPE, text=True
    )
    if out.returncode != 0:
        sys.exit(out.returncode)
    data = json.loads(out.stdout)
    for f in data["data"][0]["files"]:
        for s in f.get("segments", []):
            if len(s) >= 5 and s[2] == 0 and s[3] and s[4]:
                print(f"{f['filename']}:{s[0]}:{s[1]}")

# Show uncovered functions from JSON coverage data
cov-functions *args:
    #!/usr/bin/env python3
    import json, subprocess, sys
    src_prefix = "{{justfile_directory()}}/src/"
    out = subprocess.run(
        ["cargo", "llvm-cov", "--json"] + "{{args}}".split(),
        stdout=subprocess.PIPE, text=True
    )
    if out.returncode != 0:
        sys.exit(out.returncode)
    data = json.loads(out.stdout)
    for fn in data["data"][0]["functions"]:
        if fn["count"] == 0:
            filenames = [f for f in fn.get("filenames", []) if f.startswith(src_prefix)]
            if filenames:
                import re
                name = re.sub(r'_R.*?_\d+', '', fn["name"])
                print(f"{filenames[0]}: {fn['name']}")

# Format, check, and lint
check *args:
    cargo fmt {{args}}
    cargo check {{args}}
    cargo clippy --all-targets -- -D warnings {{args}}

# Clean generated output (gambit_out, lcov.info)
clean:
    rm -rfv gambit_out lcov.info tests/fixtures/*/gambit_out tests/fixtures/*/out tests/fixtures/*/cache

# Run dregs on the simple fixture
example:
    cargo run -- run --project tests/fixtures/simple

fmt:
    cargo fmt
    git ls-files | xargs prettier -w --ignore-unknown
