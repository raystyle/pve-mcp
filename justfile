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

# The full check gate
ci: fmt lint test test-doc

# Release build (thin-LTO)
build:
    cargo build --release --workspace
