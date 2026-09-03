use std::fmt::Write as _;

use crate::model::{AuditReport, ComparisonReport, Outcome, OutputFormat};

fn percent(value: f64) -> String {
    format!("{:.2}%", value * 100.0)
}

fn number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "undefined".to_owned())
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn outcome_icon(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "PASS",
        Outcome::Warn => "WARN",
        Outcome::Fail => "FAIL",
    }
}

fn comparison_markdown(output: &mut String, report: &ComparisonReport) {
    let _ = writeln!(
        output,
        "## {} vs baseline\n",
        escape_cell(&report.candidate)
    );
    let _ = writeln!(output, "| Metric | Value |");
    let _ = writeln!(output, "|---|---:|");
    let _ = writeln!(output, "| Shared keys | {} |", report.keys.shared);
    let _ = writeln!(
        output,
        "| Baseline-only keys | {} |",
        report.keys.baseline_only
    );
    let _ = writeln!(
        output,
        "| Candidate-only keys | {} |",
        report.keys.candidate_only
    );
    let _ = writeln!(output, "| Key Jaccard | {:.6} |", report.keys.jaccard);
    let _ = writeln!(output, "| MAE | {:.6} |", report.numeric.mae);
    let _ = writeln!(output, "| RMSE | {:.6} |", report.numeric.rmse);
    let _ = writeln!(output, "| Pearson | {} |", number(report.numeric.pearson));
    let _ = writeln!(output, "| Spearman | {} |", number(report.numeric.spearman));
    let _ = writeln!(
        output,
        "| p50 / p95 / max absolute delta | {:.6} / {:.6} / {:.6} |",
        report.numeric.p50_abs_delta, report.numeric.p95_abs_delta, report.numeric.max_abs_delta
    );
    let _ = writeln!(
        output,
        "| Strict sign flips | {} / {} ({}) |",
        report.numeric.strict_sign_flips,
        report.numeric.compared,
        percent(report.numeric.strict_sign_flip_rate)
    );
    let status_rate = report
        .status
        .disagreement_rate
        .map(percent)
        .unwrap_or_else(|| "undefined".to_owned());
    let _ = writeln!(
        output,
        "| Status disagreements | {} / {} ({}) |",
        report.status.disagreements, report.status.comparable, status_rate
    );
    let _ = writeln!(
        output,
        "| Top-{} overlap | {} / union {} (Jaccard {:.6}) |\n",
        report.top_k.requested_k,
        report.top_k.overlap,
        report.top_k.baseline_selected + report.top_k.candidate_selected - report.top_k.overlap,
        report.top_k.jaccard
    );

    if !report.largest_changes.is_empty() {
        let _ = writeln!(output, "### Largest shared-key changes\n");
        let _ = writeln!(
            output,
            "| Sample | Feature | Baseline | Candidate | Delta | Baseline status | Candidate status |"
        );
        let _ = writeln!(output, "|---|---|---:|---:|---:|---|---|");
        for change in &report.largest_changes {
            let _ = writeln!(
                output,
                "| {} | {} | {:.6} | {:.6} | {:+.6} | {} | {} |",
                escape_cell(&change.sample_id),
                escape_cell(&change.feature_id),
                change.baseline_value,
                change.candidate_value,
                change.delta,
                change
                    .baseline_status
                    .as_deref()
                    .map(escape_cell)
                    .unwrap_or_else(|| "—".to_owned()),
                change
                    .candidate_status
                    .as_deref()
                    .map(escape_cell)
                    .unwrap_or_else(|| "—".to_owned())
            );
        }
        output.push('\n');
    }

    if !report.most_affected_samples.is_empty() {
        let _ = writeln!(output, "### Most affected samples\n");
        let _ = writeln!(
            output,
            "| Sample | Shared features | Key Jaccard | MAE | p95 abs delta | Sign flips | Status changes |"
        );
        let _ = writeln!(output, "|---|---:|---:|---:|---:|---:|---:|");
        for sample in &report.most_affected_samples {
            let _ = writeln!(
                output,
                "| {} | {} | {:.6} | {:.6} | {:.6} | {} | {} |",
                escape_cell(&sample.sample_id),
                sample.shared_features,
                sample.key_jaccard,
                sample.mae,
                sample.p95_abs_delta,
                sample.strict_sign_flips,
                sample.status_disagreements
            );
        }
        output.push('\n');
    }
}

pub fn render_markdown(report: &AuditReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "# refaudit report\n");
    let _ = writeln!(output, "**Outcome: {}**\n", outcome_icon(report.outcome));
    let _ = writeln!(output, "- Baseline: `{}`", report.baseline);
    let _ = writeln!(output, "- Input: `{}`", report.input.source);
    let _ = writeln!(output, "- Records: {}", report.input.records);
    let _ = writeln!(
        output,
        "- References: {}",
        report.input.references.join(", ")
    );
    let _ = writeln!(output, "- Samples: {}", report.input.samples);
    let _ = writeln!(output, "- Features: {}\n", report.input.features);

    if report.findings.is_empty() {
        let _ = writeln!(output, "## Findings\n\nNo instability findings.\n");
    } else {
        let _ = writeln!(output, "## Findings\n");
        let _ = writeln!(output, "| Severity | Code | Comparison | Detail |");
        let _ = writeln!(output, "|---|---|---|---|");
        for finding in &report.findings {
            let _ = writeln!(
                output,
                "| {:?} | `{}` | {} | {} |",
                finding.severity,
                finding.code,
                escape_cell(&finding.comparison),
                escape_cell(&finding.message)
            );
        }
        output.push('\n');
    }

    for comparison in &report.comparisons {
        comparison_markdown(&mut output, comparison);
    }

    let _ = writeln!(
        output,
        "---\nGenerated by refaudit {} using schema {}. Metrics describe output stability, not biological correctness.",
        report.tool.version, report.schema_version
    );
    output
}

pub fn render(report: &AuditReport, format: OutputFormat) -> Result<String, serde_json::Error> {
    match format {
        OutputFormat::Markdown => Ok(render_markdown(report)),
        OutputFormat::Json => serde_json::to_string_pretty(report).map(|mut json| {
            json.push('\n');
            json
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markdown_table_cells() {
        assert_eq!(escape_cell("a|b\nc"), "a\\|b c");
    }
}
