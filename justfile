default:
    @just --list

# Format check (does not modify)
fmt:
    cargo fmt --all -- --check

# Lints: zero warnings, all targets
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Test suite via nextest (process isolation; env/cwd-sensitive tests break in
# cargo test's threaded model)
test:
    cargo nextest run --workspace

# Doc tests (nextest does not run them)
test-doc:
    cargo test --doc --workspace

# Dev-system consistency lint (asset graph: docs↔facts, ADR index, hooks,
# skills, terminology, credential red line). Node layer — checks clippy
# cannot express. Precedent: oxc tsrs source.mjs (Node lints alongside
# clippy), vulnforge system-lint (asset graph).
system-lint:
    node tools/system-lint.mjs

# The full check gate
ci: fmt lint test test-doc system-lint

# Release build (thin-LTO)
build:
    cargo build --release --workspace
