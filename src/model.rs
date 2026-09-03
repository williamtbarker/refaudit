use std::collections::BTreeMap;

use serde::Serialize;

/// A stable key shared across reference-specific outputs.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RecordKey {
    pub sample_id: String,
    pub feature_id: String,
}

/// One observed downstream value.
#[derive(Clone, Debug)]
pub struct Observation {
    pub value: f64,
    pub status: Option<String>,
}

/// Validated input grouped by reference and then by stable key.
#[derive(Clone, Debug)]
pub struct Dataset {
    pub references: BTreeMap<String, BTreeMap<RecordKey, Observation>>,
    pub record_count: usize,
    pub samples: Vec<String>,
    pub features: Vec<String>,
    pub has_status: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Markdown,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailOn {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Outcome {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditReport {
    pub schema_version: &'static str,
    pub tool: ToolInfo,
    pub input: InputSummary,
    pub baseline: String,
    pub settings: SettingsSummary,
    pub outcome: Outcome,
    pub comparisons: Vec<ComparisonReport>,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct InputSummary {
    pub source: String,
    pub records: usize,
    pub references: Vec<String>,
    pub samples: usize,
    pub features: usize,
    pub status_column_present: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SettingsSummary {
    pub epsilon: f64,
    pub top_k: usize,
    pub max_examples: usize,
    pub gates: GateSummary,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GateSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_key_jaccard: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_spearman: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sign_flip_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_status_disagreement_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_p95_abs_delta: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComparisonReport {
    pub candidate: String,
    pub keys: KeyMetrics,
    pub numeric: NumericMetrics,
    pub status: StatusMetrics,
    pub top_k: TopKMetrics,
    pub largest_changes: Vec<KeyChange>,
    pub most_affected_samples: Vec<SampleChange>,
}

#[derive(Clone, Debug, Serialize)]
pub struct KeyMetrics {
    pub baseline: usize,
    pub candidate: usize,
    pub shared: usize,
    pub baseline_only: usize,
    pub candidate_only: usize,
    pub union: usize,
    pub jaccard: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumericMetrics {
    pub compared: usize,
    pub mae: f64,
    pub rmse: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pearson: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spearman: Option<f64>,
    pub p50_abs_delta: f64,
    pub p95_abs_delta: f64,
    pub max_abs_delta: f64,
    pub strict_sign_flips: usize,
    pub strict_sign_flip_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusMetrics {
    pub comparable: usize,
    pub disagreements: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disagreement_rate: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TopKMetrics {
    pub requested_k: usize,
    pub baseline_selected: usize,
    pub candidate_selected: usize,
    pub overlap: usize,
    pub jaccard: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct KeyChange {
    pub sample_id: String,
    pub feature_id: String,
    pub baseline_value: f64,
    pub candidate_value: f64,
    pub delta: f64,
    pub abs_delta: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SampleChange {
    pub sample_id: String,
    pub shared_features: usize,
    pub key_jaccard: f64,
    pub mae: f64,
    pub p95_abs_delta: f64,
    pub strict_sign_flips: usize,
    pub status_disagreements: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: &'static str,
    pub comparison: String,
    pub message: String,
}
