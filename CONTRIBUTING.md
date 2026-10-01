# Contributing to MossRS

Thank you for contributing to **MossRS**, the Rust binding layer for Moss Framework.

MossRS provides a Rust-facing interface over Moss while keeping the native boundary explicit. Safety, ownership, lifetime modelling, FFI correctness, and cross-platform builds are therefore central to the project.

## Before You Start

For public Rust API, unsafe code, FFI, ownership, or ABI changes, open an issue before substantial implementation.

Please search existing issues and pull requests before starting work.

## Development Requirements

- Git
- Rust toolchain
- Cargo
- A suitable C/C++ toolchain
- C++17
- A local Moss Framework checkout
- Platform dependencies required by Moss

The repository includes `rust-toolchain.toml`; use the pinned/project-defined Rust toolchain where applicable.

## Building and Testing

```bash
git clone https://github.com/TxbiG/MossRS.git
cd MossRS

cargo build
cargo test
```

Where the project configuration supports them, also run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Repository Structure

- `crates/moss-sys/` — low-level native FFI layer
- `docs/` — documentation
- `examples/platformer/` — Rust example
- `performance/` — benchmarks
- `.github/workflows/` — CI
- `Cargo.toml` — workspace/package configuration

## Areas for Contribution

- Safe Rust API
- FFI coverage
- Native handle wrappers
- Ownership/lifetime modelling
- Error handling
- Math/vector types
- Rendering
- Audio
- Physics
- Input
- XR
- Examples
- Documentation
- Performance
- Cross-platform CI

## Rust Safety Guidelines

Keep unsafe code concentrated in the FFI layer where practical.

A safe wrapper should establish and document its invariants before exposing operations through safe Rust.

Pay particular attention to:

- Pointer validity
- Ownership
- Lifetimes
- Thread safety
- `Send`/`Sync` guarantees
- Resource destruction
- C-string conversion
- Alignment
- Struct layout
- ABI compatibility

Do not mark a type `Send` or `Sync` unless the underlying Moss resource is actually safe to use that way.

## FFI Guidelines

Keep the low-level `moss-sys` layer close to the native ABI.

Higher-level Rust wrappers should provide safer and more idiomatic interfaces without silently changing Moss semantics.

When changing native structures or functions, verify:

- Layout
- Calling convention
- Integer widths
- Floating-point representation
- Ownership
- Nullability

## Testing

Test normal and invalid resource lifetimes, including failure paths.

When changing unsafe code, include tests or validation that exercise the invariants relied upon by the wrapper.

## Examples

When adding a significant public feature, add or update a small example where practical. Examples should demonstrate intended public usage rather than internal implementation details.

## Commit Messages

Recommended prefixes:

```text
feat: add safe texture wrapper
fix: prevent invalid native handle use
docs: document Moss window ownership
test: add FFI lifetime regression
perf: reduce vector conversion overhead
refactor: isolate unsafe FFI code
```

## Pull Requests

Include:

- What changed
- Safety considerations
- Tests performed
- Rust/toolchain version
- Target platform
- Native Moss version/commit
- API compatibility notes

For `unsafe` changes, explain the invariants that make the code sound.

## Licence

MossRS is distributed under the **MIT License**. Contributions should be compatible with the repository's licence and applicable Rust/Moss/third-party licence requirements.

Thank you for contributing.
