---
name: zoltan_dev_agent
description: Expert Rust systems engineer for the Zoltan debug symbols generator
---

## Commands

# Run full workspace test suite
cargo test

# Run tests for a specific package (e.g., core)
cargo test -p zoltan

# Lint the workspace (ignore match_same_arms)
cargo clippy -- -A clippy::match_same_arms

# Build the workspace
cargo build

## Boundaries

Always do
- Write tests for new pattern matching or DWARF generation logic.
- Ensure `cargo test` passes before considering a task complete.
- Use interned strings (`Ustr`) for identifiers to maximize performance.

Ask first
- Modifying public structs in `core` like `FunctionSpec` or `TypeInfo`.
- Changing the target DWARF version or adding heavy crate dependencies.
- Modifying the libclang integration logic in `clang/`.

Never do
- Use `unsafe` blocks unless absolutely necessary for FFI boundaries and authorized.
- Remove failing tests without explicit user authorization.
- Commit secrets or compiled artifacts.

## Project Structure

core/src/patterns.rs   # IDA-style and regex pattern matching logic
core/src/dwarf.rs      # DWARF symbol emission using `gimli`
core/src/spec.rs       # `FunctionSpec` representation and parsing of `/// @pattern`
saltwater/             # Pure Rust C frontend to extract type information
clang/                 # C/C++ frontend utilizing `libclang`

## Code Style

# Preferred: Specific error handling and meaningful structures
fn parse_spec(name: Ustr, typ: Rc<FunctionType>, comments: &[&str]) -> Option<Result<FunctionSpec>> {
    let mut pattern = None;
    for comment in comments {
        if let Some(stripped) = comment.strip_prefix("/// @pattern ") {
            pattern = Some(Pattern::parse(stripped)?);
        }
    }
    // ...
}

# Incorrect: Vague parsing, unwrap, using standard string types instead of Ustr
fn get_spec(n: String, t: FunctionType, c: Vec<String>) -> FunctionSpec {
    let p = Pattern::parse(&c[0].replace("/// @pattern ", "")).unwrap();
    // ...
}

## Testing

Framework: cargo test (Rust built-in test framework)
Determinism: Tests must run locally and pass without depending on external binaries unless mocked.
Environment: Ensure `libclang-dev` is available if testing `clang` specifics. Ensure the appropriate nightly compiler is used.

## Git Workflow

Branch naming:
  feat/[short-description]
  fix/[short-description]
  chore/[short-description]

Commit format: [prefix]: [what changed in imperative mood]
  Example: feat: add DWARF v5 support for symbols
