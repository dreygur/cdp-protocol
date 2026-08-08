node_dir := "crates/cdp-protocol-node"

# List available recipes
default:
    @just --list

# Install git hooks + node deps
setup:
    git config core.hooksPath .githooks
    cd {{node_dir}} && npm ci

# Format Rust
fmt:
    cargo fmt --all

# Check formatting (CI-safe)
fmt-check:
    cargo fmt --all -- --check

# Clippy, warnings are errors
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Rust unit tests
test:
    cargo test -p cdp-driver
    cargo test -p xtask

# Regenerate src/methods/ from protocol/*.json (rustfmt wraps the long ones)
generate-methods:
    cargo run -p xtask
    cargo fmt --all

# Refresh the vendored CDP schemas from upstream, then regenerate
update-protocol:
    curl -sfL -o protocol/browser_protocol.json https://raw.githubusercontent.com/ChromeDevTools/devtools-protocol/master/json/browser_protocol.json
    curl -sfL -o protocol/js_protocol.json https://raw.githubusercontent.com/ChromeDevTools/devtools-protocol/master/json/js_protocol.json
    just generate-methods

# Node smoke test (needs Chrome on :9222)
test-node:
    cd {{node_dir}} && npm test

# Release build
build:
    cargo build --release

# Build napi bindings
build-node:
    cd {{node_dir}} && npm run build

# Usage: just dev [name]   where name = basic | agent | cluster | industrial
# Hot-reload a Rust example on save (needs watchexec + Chrome on :9222)
dev name="basic":
    watchexec -w crates/cdp-protocol -e rs -r -- cargo run -p cdp-driver --example {{ name }}

# Dependency advisories (needs cargo-audit)
audit:
    cargo audit

# What pre-push / CI should run
ci: fmt-check lint test

# Remove build artifacts
clean:
    cargo clean
    cd {{node_dir}} && rm -f *.node
