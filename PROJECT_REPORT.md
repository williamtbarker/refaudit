# Project report: refaudit 0.1.0

## Selection rationale

This release candidate was chosen after screening several current bioinformatics needs for novelty, feasibility, and the risk of overstating results.

| Candidate | Evidence of need | Prior-art / risk screen | Decision |
|---|---|---|---|
| Influenza primer-drift monitor | Assay mismatches remain operationally important | Mature tools already include ViralPrimer, Primer Monitor, PriSM, and FEVER/PSET; in-silico mismatch effects also require careful wet-lab interpretation | Reject |
| Influenza segmented-genome bundle QC | Segment completeness and metadata integrity matter | GenoFLU, BV-BRC TreeSort, NCBI grouping, VADR, and established flu workflows cover much of the space | Reject |
| Paper/code claim consistency | Reproducibility is a current research focus | BioCon, SciCoQA, CSPaper Code Check, TCTracer, and related work make this crowded | Reject |
| Downstream reference-choice stability audit | Multiple 2026 studies show reference-dependent shifts in functional genomics and transcriptomics | Existing tools largely perform mapping, liftover, or study-specific analyses; no focused local comparator for already-produced matched result tables was found in the screen | Select |

The selected scope is narrow on purpose. `refaudit` neither recreates a bioinformatics pipeline nor pretends to choose a biologically correct reference. It makes one under-served reproducibility check easy to run and easy to gate in CI.

## Research trigger

The primary trigger was *Benchmarking genome choice in functional genomics analyses* (Nature Communications, 2026), which compares identical datasets processed against CHM13, hg38, personalized assemblies, and pangenomic methods across ATAC-seq, RNA-seq, WGBS, and Hi-C. The study reports assay-dependent reference effects on QC and downstream analyses.

A complementary 2026 Genome Biology study shows that reference and annotation updates can alter gene quantification, differential-expression membership, and the direction of some Alzheimer’s disease associations even when broader signatures remain concordant.

These papers motivate sensitivity analysis. They do not establish universal pass/fail thresholds, which is why all scientific gates in `refaudit` are opt-in.

## Differentiation

`refaudit` operates after upstream processing and accepts generic numeric and categorical outputs. This makes it:

- pipeline-agnostic rather than tied to one aligner, assay, organism, or file format;
- suitable for a homogeneous table of QC values, abundance estimates, effect sizes, scores, or other harmonized outputs;
- deterministic and offline;
- explicit about missingness, undefined correlations, and duplicate-key errors;
- safe for CI because malformed input and policy failure have distinct exit codes.

The main tradeoff is that semantic harmonization remains the user’s responsibility. Genericity is useful only if the comparison key genuinely identifies the same concept under each reference.

## Threat model and scientific limits

The tool does not execute input content or access the network. It treats all rows as untrusted data, rejects non-finite values and duplicate keys, and bounds retained diagnostic examples. The full dataset must still fit in memory.

The audit cannot detect:

- incorrect or non-equivalent feature identifiers;
- sample swaps that preserve identifiers;
- upstream pipeline changes confounded with reference changes;
- biological truth or clinical relevance;
- coordinate equivalence or orthology.

Aggregate numeric metrics are meaningful only within one common unit and scale. The input contract therefore requires heterogeneous measures to be partitioned into separate audits.

## Release criteria

- deterministic Markdown and JSON output;
- strict schema and duplicate validation;
- content-based gzip detection;
- tests for stable, warning, gate-failure, malformed, gzip, custom-schema, and file-output paths;
- formatting and Clippy clean with warnings denied;
- release build and clean archive verification;
- documented assumptions, metric definitions, and exit codes.

## References

1. [Benchmarking genome choice in functional genomics analyses](https://doi.org/10.1038/s41467-026-73663-3), *Nature Communications* (2026).
2. [Updates to the reference genome alter the detection and direction of genes differentially expressed in Alzheimer's disease](https://doi.org/10.1186/s13059-026-04213-9), *Genome Biology* (2026).
3. [One is not enough: On the effects of reference genome for the mapping and subsequent analyses of short-reads](https://doi.org/10.1371/journal.pcbi.1008678), *PLOS Computational Biology* (2021).
4. [Exome variant discrepancies due to reference-genome differences](https://doi.org/10.1016/j.ajhg.2021.05.011), *American Journal of Human Genetics* (2021).
