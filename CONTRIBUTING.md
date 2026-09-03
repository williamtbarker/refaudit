# Contributing

Small, reviewable changes are welcome.

## Setup

Install Rust 1.85 or newer, clone the repository, and run:

```bash
make verify
```

## Expectations

- Add tests for changed behavior, including failure paths.
- Keep output deterministic; use ordered collections or explicit tie-breakers.
- Do not introduce assay-specific biological thresholds as defaults.
- Preserve the distinction between malformed input (exit 1) and policy failure (exit 2).
- Document any metric definition or schema change and update `schema_version` when compatibility changes.
- Run `cargo fmt` before opening a pull request and keep Clippy warning-free.

Bug reports should include the command, tool version, a minimal de-identified input, expected behavior, and actual output. Do not attach protected health information or confidential sample metadata.
