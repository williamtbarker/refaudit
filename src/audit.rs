use std::path::Path;

use thiserror::Error;

use crate::metrics::{
    key_metrics, largest_changes, most_affected_samples, numeric_metrics, status_metrics,
    top_k_metrics,
};
use crate::model::{
    AuditReport, ComparisonReport, Dataset, Finding, GateSummary, InputSummary, Outcome,
    SettingsSummary, Severity, ToolInfo,
};

#[derive(Clone, Debug)]
pub struct AuditConfig {
    pub baseline: String,
    pub epsilon: f64,
    pub top_k: usize,
    pub max_examples: usize,
    pub gates: GateSummary,
}

impl AuditConfig {
    pub fn validate(&self) -> Result<(), AuditError> {
        if !self.epsilon.is_finite() || self.epsilon < 0.0 {
            return Err(AuditError::InvalidConfig(
                "--epsilon must be a finite number greater than or equal to 0".to_owned(),
            ));
        }
        if self.top_k == 0 {
            return Err(AuditError::InvalidConfig(
                "--top-k must be greater than 0".to_owned(),
            ));
        }
        if self.max_examples == 0 {
            return Err(AuditError::InvalidConfig(
                "--max-examples must be greater than 0".to_owned(),
            ));
        }
        for (name, value) in [
            ("--min-key-jaccard", self.gates.min_key_jaccard),
            ("--min-spearman", self.gates.min_spearman),
            ("--max-sign-flip-rate", self.gates.max_sign_flip_rate),
            (
                "--max-status-disagreement-rate",
                self.gates.max_status_disagreement_rate,
            ),
        ] {
            if let Some(value) = value {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(AuditError::InvalidConfig(format!(
                        "{name} must be between 0 and 1"
                    )));
                }
            }
        }
        if let Some(value) = self.gates.max_p95_abs_delta {
            if !value.is_finite() || value < 0.0 {
                return Err(AuditError::InvalidConfig(
                    "--max-p95-abs-delta must be a finite number greater than or equal to 0"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("baseline reference {baseline:?} was not found; available references: {available}")]
    BaselineNotFound { baseline: String, available: String },
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

fn warning(code: &'static str, comparison: &str, message: String) -> Finding {
    Finding {
        severity: Severity::Warning,
        code,
        comparison: comparison.to_owned(),
        message,
    }
}

fn error(code: &'static str, comparison: &str, message: String) -> Finding {
    Finding {
        severity: Severity::Error,
        code,
        comparison: comparison.to_owned(),
        message,
    }
}

pub fn run_audit(
    dataset: &Dataset,
    source: &Path,
    config: &AuditConfig,
) -> Result<AuditReport, AuditError> {
    config.validate()?;
    let baseline =
        dataset
            .references
            .get(&config.baseline)
            .ok_or_else(|| AuditError::BaselineNotFound {
                baseline: config.baseline.clone(),
                available: dataset
                    .references
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;

    let mut comparisons = Vec::new();
    let mut findings = Vec::new();
    for (candidate_name, candidate) in &dataset.references {
        if candidate_name == &config.baseline {
            continue;
        }
        let keys = key_metrics(baseline, candidate);
        let numeric = numeric_metrics(baseline, candidate, config.epsilon);
        let status = status_metrics(baseline, candidate);
        let top_k = top_k_metrics(baseline, candidate, config.top_k);

        if keys.baseline_only > 0 || keys.candidate_only > 0 {
            findings.push(warning(
                "KEY_SET_DIFFERENCE",
                candidate_name,
                format!(
                    "{} baseline-only and {} candidate-only keys",
                    keys.baseline_only, keys.candidate_only
                ),
            ));
        }
        if numeric.max_abs_delta > 0.0 {
            findings.push(warning(
                "NUMERIC_VALUES_DIFFER",
                candidate_name,
                format!(
                    "shared values differ (MAE {:.6}; p95 absolute delta {:.6}; max {:.6})",
                    numeric.mae, numeric.p95_abs_delta, numeric.max_abs_delta
                ),
            ));
        }
        if numeric.strict_sign_flips > 0 {
            findings.push(warning(
                "STRICT_SIGN_FLIPS",
                candidate_name,
                format!(
                    "{} of {} shared values cross from below -epsilon to above +epsilon or vice versa",
                    numeric.strict_sign_flips, numeric.compared
                ),
            ));
        }
        if status.disagreements > 0 {
            findings.push(warning(
                "STATUS_DISAGREEMENTS",
                candidate_name,
                format!(
                    "{} of {} comparable statuses differ",
                    status.disagreements, status.comparable
                ),
            ));
        }

        if let Some(minimum) = config.gates.min_key_jaccard {
            if keys.jaccard < minimum {
                findings.push(error(
                    "KEY_JACCARD_BELOW_MIN",
                    candidate_name,
                    format!(
                        "key Jaccard {:.6} is below minimum {:.6}",
                        keys.jaccard, minimum
                    ),
                ));
            }
        }
        if let Some(minimum) = config.gates.min_spearman {
            match numeric.spearman {
                Some(value) if value >= minimum => {}
                Some(value) => findings.push(error(
                    "SPEARMAN_BELOW_MIN",
                    candidate_name,
                    format!("Spearman {:.6} is below minimum {:.6}", value, minimum),
                )),
                None => findings.push(error(
                    "SPEARMAN_UNDEFINED",
                    candidate_name,
                    "Spearman correlation is undefined for fewer than two shared values or constant ranks"
                        .to_owned(),
                )),
            }
        }
        if let Some(maximum) = config.gates.max_sign_flip_rate {
            if numeric.strict_sign_flip_rate > maximum {
                findings.push(error(
                    "SIGN_FLIP_RATE_ABOVE_MAX",
                    candidate_name,
                    format!(
                        "strict sign-flip rate {:.6} is above maximum {:.6}",
                        numeric.strict_sign_flip_rate, maximum
                    ),
                ));
            }
        }
        if let Some(maximum) = config.gates.max_status_disagreement_rate {
            match status.disagreement_rate {
                Some(value) if value <= maximum => {}
                Some(value) => findings.push(error(
                    "STATUS_DISAGREEMENT_RATE_ABOVE_MAX",
                    candidate_name,
                    format!(
                        "status-disagreement rate {:.6} is above maximum {:.6}",
                        value, maximum
                    ),
                )),
                None => findings.push(error(
                    "STATUS_DISAGREEMENT_RATE_UNDEFINED",
                    candidate_name,
                    "status-disagreement rate is undefined because no shared keys have statuses in both references"
                        .to_owned(),
                )),
            }
        }
        if let Some(maximum) = config.gates.max_p95_abs_delta {
            if numeric.p95_abs_delta > maximum {
                findings.push(error(
                    "P95_ABS_DELTA_ABOVE_MAX",
                    candidate_name,
                    format!(
                        "p95 absolute delta {:.6} is above maximum {:.6}",
                        numeric.p95_abs_delta, maximum
                    ),
                ));
            }
        }

        comparisons.push(ComparisonReport {
            candidate: candidate_name.clone(),
            keys,
            numeric,
            status,
            top_k,
            largest_changes: largest_changes(baseline, candidate, config.max_examples),
            most_affected_samples: most_affected_samples(
                baseline,
                candidate,
                config.epsilon,
                config.max_examples,
            ),
        });
    }

    findings.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.comparison.cmp(&right.comparison))
            .then_with(|| left.code.cmp(right.code))
    });
    let outcome = if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Outcome::Fail
    } else if findings.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Warn
    };

    Ok(AuditReport {
        schema_version: "1.0.0",
        tool: ToolInfo {
            name: "refaudit",
            version: env!("CARGO_PKG_VERSION"),
        },
        input: InputSummary {
            source: source.display().to_string(),
            records: dataset.record_count,
            references: dataset.references.keys().cloned().collect(),
            samples: dataset.samples.len(),
            features: dataset.features.len(),
            status_column_present: dataset.has_status,
        },
        baseline: config.baseline.clone(),
        settings: SettingsSummary {
            epsilon: config.epsilon,
            top_k: config.top_k,
            max_examples: config.max_examples,
            gates: config.gates.clone(),
        },
        outcome,
        comparisons,
        findings,
    })
}
