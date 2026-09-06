# refaudit

[![CI](https://github.com/williamtbarker/refaudit/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/williamtbarker/refaudit/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/williamtbarker/refaudit)](https://github.com/williamtbarker/refaudit/releases) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

`refaudit` is a small, offline Rust CLI for answering a reproducibility question that is easy to miss:

> If I process the same biological samples against a different reference genome, do my downstream results materially change?

Give it one long-format CSV or TSV containing one comparable numeric measure from two or more references. It compares every candidate with your chosen baseline and produces a deterministic Markdown or JSON report with key coverage, numeric agreement, rank stability, strict sign flips, categorical status changes, top-k overlap, and the samples or features most affected.

It does **not** align reads, lift coordinates, infer orthology, or decide which reference is correct. It audits outputs that your validated pipelines have already produced.

## Why this exists

Recent functional-genomics work shows that reference choice can systematically shift QC metrics and downstream results even when the experimental data are unchanged. A second 2026 transcriptomics study found that genome and annotation updates can change which disease-associated genes are detected and even the direction of some associations. `refaudit` turns that methodological concern into a lightweight check that can run beside existing workflows.

- [Benchmarking genome choice in functional genomics analyses](https://doi.org/10.1038/s41467-026-73663-3), *Nature Communications* (2026)
- [Updates to the reference genome alter the detection and direction of genes differentially expressed in Alzheimer's disease](https://doi.org/10.1186/s13059-026-04213-9), *Genome Biology* (2026)

This project is independent of those studies and does not reproduce their analyses.

## Quick start

Requires Rust 1.83 or newer.

```bash
cargo install --path . --locked
refaudit audit \
  --input examples/reference_sensitive.tsv \
  --baseline hg38 \
  --output report.md
```

Or run without installing:

```bash
cargo run --locked -- audit \
  --input examples/stable.tsv \
  --baseline hg38 \
  --format json
```

`examples/stable.tsv` produces `PASS`. `examples/reference_sensitive.tsv` intentionally produces `WARN` while returning exit code 0, so you can inspect real differences before choosing domain-specific gates.

## Input contract

The default schema is:

| Column | Required | Meaning |
|---|---:|---|
| `reference` | yes | Reference genome/build label |
| `sample_id` | yes | Stable sample identifier |
| `feature_id` | yes | Stable, harmonized metric or feature identifier |
| `value` | yes | Finite numeric result in one consistent unit/scale |
| `status` | no | Optional categorical call such as `pass`, `significant`, or `detected` |

Each `(reference, sample_id, feature_id)` tuple must be unique. Duplicate keys are rejected rather than silently aggregated. Status comparison is ASCII case-insensitive and ignores surrounding whitespace.

One audit must contain one meaningfully comparable measure, such as log2 fold change, normalized abundance, or one QC score. Do not mix unlike units or scales in the same input: MAE, correlation, ranking, and top-k statistics would not be interpretable. Partition heterogeneous pipeline metrics into separate audits.

CSV, TSV, and gzip-compressed inputs are supported. `.tsv`, `.tab`, and their `.gz` variants are inferred as tab-delimited; other names default to CSV. Gzip is detected by magic bytes, not the filename. Column names and the delimiter can be overridden:

```bash
refaudit audit \
  --input metrics.txt.gz \
  --delimiter tsv \
  --baseline GRCh38 \
  --reference-column build \
  --sample-column specimen \
  --feature-column metric \
  --value-column estimate \
  --status-column decision
```

### The important precondition

`feature_id` values must already refer to comparable entities. For example, gene IDs should be harmonized across annotation releases, and coordinate-specific features should be lifted or otherwise reconciled before auditing. A missing key may reflect a real reference effect, a changed annotation, or inadequate harmonization; `refaudit` reports the difference but cannot distinguish those causes.

## Metrics

For each candidate reference, `refaudit` reports:

- shared, baseline-only, and candidate-only keys plus key-set Jaccard;
- mean absolute error (MAE), root mean squared error (RMSE), and absolute-delta p50/p95/max;
- Pearson and tie-aware Spearman correlations when mathematically defined;
- strict sign flips across `-epsilon` and `+epsilon`;
- disagreements where both rows contain a status;
- Jaccard overlap among the top-k absolute values;
- largest shared-key changes and most affected samples.

Quantiles use linear interpolation at position `(n - 1) × p`. Top-k ties are broken deterministically by `(sample_id, feature_id)`. Missing keys are included in key and top-k metrics but excluded from numeric and status comparisons.

## Gates and CI behavior

There are no universal biological tolerances, so report-only mode is the default. Add gates that make sense for your assay and value scale:

```bash
refaudit audit \
  --input results.tsv.gz \
  --baseline hg38 \
  --min-key-jaccard 0.98 \
  --min-spearman 0.95 \
  --max-sign-flip-rate 0.01 \
  --max-status-disagreement-rate 0.02 \
  --max-p95-abs-delta 0.25 \
  --format json \
  --output refaudit.json
```

| Exit code | Meaning |
|---:|---|
| `0` | Valid input; no configured gate failed. Warnings may still be present. |
| `1` | Input, configuration, serialization, or output error. |
| `2` | A configured gate failed, or `--fail-on warning` promoted an observed warning. |

The report is written before exit code 2, which lets CI retain the diagnostic artifact. Use `--fail-on warning` when any numeric change, missing key, strict sign flip, or status disagreement should stop a pipeline. By default, exact numeric equality is required to avoid `NUMERIC_VALUES_DIFFER`; use explicit gates instead when harmless floating-point variation is expected.

## Interpretation guardrails

- High global correlation can coexist with consequential changes in a small set of features; inspect the examples and status changes.
- A `PASS` means the supplied tables met the supplied policy. It is not evidence that either reference is correct or that the upstream analysis is valid.
- A `WARN` is descriptive, not a declaration of biological significance.
- MAE and absolute-delta gates are scale-dependent. Do not reuse thresholds across unlike metrics without normalization.
- Never combine unlike metrics in one audit; run separate audits for separate units or measurement semantics.
- Strict sign flips exclude values in the neutral interval `[-epsilon, +epsilon]`; set `--epsilon` to suppress meaningless near-zero flips.
- Top-k is calculated across all sample-feature keys, not separately within each sample.

## Development

```bash
make verify
```

The verification target checks formatting, runs Clippy with warnings denied, executes all tests, creates a release build, and smoke-tests both example datasets. CI covers Linux, macOS, Windows, and the stated minimum Rust version.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development conventions and [VERIFICATION.md](VERIFICATION.md) for the release-candidate test record.

## Synthetic scale benchmark

On an Apple M2 Max, RefAudit processed one million synthetic long-format rows in a median
2.79 seconds across three runs, and each same-run repeat produced
byte-identical JSON. Median peak RSS was 442 MiB, making memory
reduction an explicit optimization target. [Method, ranges, and determinism qualification](docs/BENCHMARK_MACOS_2026-09-03.md).

## Scope and roadmap

Version 0.1 intentionally accepts one canonical long table and makes no assumptions about assay semantics. Reasonable future additions include per-feature grouping, a manifest for separate per-reference files, and HTML visualization. Coordinate liftover and biological “best reference” recommendations are deliberately out of scope.

## License

MIT
