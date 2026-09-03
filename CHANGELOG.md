# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses [Semantic Versioning](https://semver.org/).

## Unreleased

## 0.1.1 - 2026-09-03

### Fixed

- Switched to Rust Edition 2021 so Cargo 1.83 can parse the project.
- Pinned Edition-2021-compatible dependency releases in the lockfile.
- Made source formatting identical under Rustfmt 1.83 and current stable.

## 0.1.0 - 2026-09-03

### Added

- Deterministic comparisons of matched downstream values across reference genomes.
- Key coverage, numeric error, correlation, sign-flip, status, and top-k metrics.
- Per-key and per-sample diagnostic examples.
- Opt-in CI gates with distinct input-error and policy-failure exit codes.
- CSV, TSV, custom column names, and content-detected gzip support.
- Markdown and versioned JSON reports.
