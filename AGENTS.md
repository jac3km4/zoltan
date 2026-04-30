## Commands
```bash
# Build the project
cargo build

# Run linting with warnings treated as errors (required by CI)
cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
cargo test
```

## Boundaries
### Always do
- Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` before submitting a PR.

### Ask first
- Modifying `Cargo.toml` dependencies.

### Never do
- Prefer third-party actions like `dtolnay/rust-toolchain` over just invoking builtin commands like `rustup toolchain install stable --profile minimal --component clippy --no-self-update` for GitHub Actions workflows.

## Project Structure
```text
core/src/patterns.rs   # IDA-style and regex pattern matching logic
core/src/dwarf.rs      # DWARF symbol emission using `gimli`
core/src/spec.rs       # `FunctionSpec` representation and parsing of `/// @pattern`
saltwater/             # Pure Rust C frontend to extract type information
clang/                 # C/C++ frontend utilizing `libclang`
```

## Testing
- **Framework:** `cargo test`

## Git Workflow
Branch naming:
  feat/[short-description]
  fix/[short-description]
  chore/[short-description]

Commit format: [prefix]: [what changed in imperative mood]
  Example: feat: add DWARF v5 support for symbols
