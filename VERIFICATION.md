# Verification record

This file records the checks used for release candidate 0.1.1 on 2026-09-03. The delivered ZIP is created from the staged Git tree, extracted into a fresh temporary directory, and tested again before delivery.

## Source checks

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
cargo package --locked --allow-dirty
```

| Check | Result |
|---|---|
| Stable toolchain | Rust/Cargo 1.98.0 |
| Minimum supported toolchain | Rust/Cargo 1.83.0 |
| Unit and CLI tests | 37 passed, 0 failed |
| Line coverage | 97.27% (1,035 / 1,064 instrumented lines) |
| Region coverage | 96.55% |
| Clippy | Passed with warnings denied |
| Rustdoc | Passed with warnings denied |
| Release build | Passed |
| Crate packaging | 23 files; package build verified |
| RustSec audit | No known vulnerabilities reported after scanning 1,239 advisories |

Coverage was measured with `cargo llvm-cov --all-features --workspace --summary-only`. Coverage tooling is not a project dependency.

## Exact-archive checks

The final archive verification performs all of the following from a newly extracted copy:

1. compare archive contents with the staged Git tree;
2. run formatting, Clippy, all 37 tests, release build, and both bundled smoke examples;
3. rerun all tests on Rust 1.83.0;
4. install the binary into an empty temporary Cargo root;
5. run the installed binary against both the stable and reference-sensitive examples;
6. verify `PASS`, `WARN`, and gate-failure exit behavior;
7. confirm that build products, coverage data, and Git internals are absent from the ZIP.

## Result

Passed. The SHA-256 digest is supplied alongside the delivered ZIP so it does not create a self-referential archive checksum.
