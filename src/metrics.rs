use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    KeyChange, KeyMetrics, NumericMetrics, Observation, RecordKey, SampleChange, StatusMetrics,
    TopKMetrics,
};

pub fn quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (sorted.len() - 1) as f64 * probability;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = position - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

pub fn pearson(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    let n = left.len() as f64;
    let left_mean = left.iter().sum::<f64>() / n;
    let right_mean = right.iter().sum::<f64>() / n;
    let mut covariance = 0.0;
    let mut left_variance = 0.0;
    let mut right_variance = 0.0;
    for (&left_value, &right_value) in left.iter().zip(right) {
        let left_delta = left_value - left_mean;
        let right_delta = right_value - right_mean;
        covariance += left_delta * right_delta;
        left_variance += left_delta * left_delta;
        right_variance += right_delta * right_delta;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if denominator <= f64::EPSILON {
        None
    } else {
        Some((covariance / denominator).clamp(-1.0, 1.0))
    }
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_by(|&left, &right| {
        values[left]
            .partial_cmp(&values[right])
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.cmp(&right))
    });

    let mut output = vec![0.0; values.len()];
    let mut start = 0;
    while start < indices.len() {
        let mut end = start + 1;
        while end < indices.len() && values[indices[start]] == values[indices[end]] {
            end += 1;
        }
        let average_rank = ((start + 1) as f64 + end as f64) / 2.0;
        for &index in &indices[start..end] {
            output[index] = average_rank;
        }
        start = end;
    }
    output
}

pub fn spearman(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    pearson(&ranks(left), &ranks(right))
}

fn strict_sign_flip(left: f64, right: f64, epsilon: f64) -> bool {
    (left < -epsilon && right > epsilon) || (left > epsilon && right < -epsilon)
}

pub fn key_metrics(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
) -> KeyMetrics {
    let baseline_keys: BTreeSet<_> = baseline.keys().collect();
    let candidate_keys: BTreeSet<_> = candidate.keys().collect();
    let shared = baseline_keys.intersection(&candidate_keys).count();
    let union = baseline_keys.union(&candidate_keys).count();
    KeyMetrics {
        baseline: baseline.len(),
        candidate: candidate.len(),
        shared,
        baseline_only: baseline_keys.difference(&candidate_keys).count(),
        candidate_only: candidate_keys.difference(&baseline_keys).count(),
        union,
        jaccard: if union == 0 {
            1.0
        } else {
            shared as f64 / union as f64
        },
    }
}

pub fn numeric_metrics(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
    epsilon: f64,
) -> NumericMetrics {
    let mut baseline_values = Vec::new();
    let mut candidate_values = Vec::new();
    let mut absolute_deltas = Vec::new();
    let mut squared_sum = 0.0;
    let mut sign_flips = 0;

    for (key, baseline_observation) in baseline {
        if let Some(candidate_observation) = candidate.get(key) {
            baseline_values.push(baseline_observation.value);
            candidate_values.push(candidate_observation.value);
            let delta = candidate_observation.value - baseline_observation.value;
            absolute_deltas.push(delta.abs());
            squared_sum += delta * delta;
            if strict_sign_flip(
                baseline_observation.value,
                candidate_observation.value,
                epsilon,
            ) {
                sign_flips += 1;
            }
        }
    }

    absolute_deltas.sort_by(f64::total_cmp);
    let compared = absolute_deltas.len();
    let mae = if compared == 0 {
        0.0
    } else {
        absolute_deltas.iter().sum::<f64>() / compared as f64
    };
    NumericMetrics {
        compared,
        mae,
        rmse: if compared == 0 {
            0.0
        } else {
            (squared_sum / compared as f64).sqrt()
        },
        pearson: pearson(&baseline_values, &candidate_values),
        spearman: spearman(&baseline_values, &candidate_values),
        p50_abs_delta: quantile(&absolute_deltas, 0.50),
        p95_abs_delta: quantile(&absolute_deltas, 0.95),
        max_abs_delta: absolute_deltas.last().copied().unwrap_or(0.0),
        strict_sign_flips: sign_flips,
        strict_sign_flip_rate: if compared == 0 {
            0.0
        } else {
            sign_flips as f64 / compared as f64
        },
    }
}

pub fn status_metrics(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
) -> StatusMetrics {
    let mut comparable = 0;
    let mut disagreements = 0;
    for (key, baseline_observation) in baseline {
        let Some(candidate_observation) = candidate.get(key) else {
            continue;
        };
        if let (Some(left), Some(right)) =
            (&baseline_observation.status, &candidate_observation.status)
        {
            comparable += 1;
            if left != right {
                disagreements += 1;
            }
        }
    }
    StatusMetrics {
        comparable,
        disagreements,
        disagreement_rate: (comparable > 0).then(|| disagreements as f64 / comparable as f64),
    }
}

fn top_keys(records: &BTreeMap<RecordKey, Observation>, k: usize) -> BTreeSet<RecordKey> {
    let mut ranked: Vec<_> = records.iter().collect();
    ranked.sort_by(|(left_key, left), (right_key, right)| {
        right
            .value
            .abs()
            .total_cmp(&left.value.abs())
            .then_with(|| left_key.cmp(right_key))
    });
    ranked
        .into_iter()
        .take(k)
        .map(|(key, _)| key.clone())
        .collect()
}

pub fn top_k_metrics(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
    k: usize,
) -> TopKMetrics {
    let baseline_top = top_keys(baseline, k);
    let candidate_top = top_keys(candidate, k);
    let overlap = baseline_top.intersection(&candidate_top).count();
    let union = baseline_top.union(&candidate_top).count();
    TopKMetrics {
        requested_k: k,
        baseline_selected: baseline_top.len(),
        candidate_selected: candidate_top.len(),
        overlap,
        jaccard: if union == 0 {
            1.0
        } else {
            overlap as f64 / union as f64
        },
    }
}

pub fn largest_changes(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
    limit: usize,
) -> Vec<KeyChange> {
    let mut changes = Vec::new();
    for (key, baseline_observation) in baseline {
        if let Some(candidate_observation) = candidate.get(key) {
            let delta = candidate_observation.value - baseline_observation.value;
            if delta == 0.0 && baseline_observation.status == candidate_observation.status {
                continue;
            }
            changes.push(KeyChange {
                sample_id: key.sample_id.clone(),
                feature_id: key.feature_id.clone(),
                baseline_value: baseline_observation.value,
                candidate_value: candidate_observation.value,
                delta,
                abs_delta: delta.abs(),
                baseline_status: baseline_observation.status.clone(),
                candidate_status: candidate_observation.status.clone(),
            });
            if changes.len() >= limit.saturating_mul(2) {
                sort_changes(&mut changes);
                changes.truncate(limit);
            }
        }
    }
    sort_changes(&mut changes);
    changes.truncate(limit);
    changes
}

fn sort_changes(changes: &mut [KeyChange]) {
    changes.sort_by(|left, right| {
        right
            .abs_delta
            .total_cmp(&left.abs_delta)
            .then_with(|| left.sample_id.cmp(&right.sample_id))
            .then_with(|| left.feature_id.cmp(&right.feature_id))
    });
}

pub fn most_affected_samples(
    baseline: &BTreeMap<RecordKey, Observation>,
    candidate: &BTreeMap<RecordKey, Observation>,
    epsilon: f64,
    limit: usize,
) -> Vec<SampleChange> {
    #[derive(Default)]
    struct Accumulator {
        baseline_features: usize,
        candidate_features: usize,
        shared_features: usize,
        absolute_deltas: Vec<f64>,
        strict_sign_flips: usize,
        status_disagreements: usize,
    }

    let mut by_sample = BTreeMap::<String, Accumulator>::new();
    for key in candidate.keys() {
        by_sample
            .entry(key.sample_id.clone())
            .or_default()
            .candidate_features += 1;
    }
    for (key, baseline_observation) in baseline {
        let accumulator = by_sample.entry(key.sample_id.clone()).or_default();
        accumulator.baseline_features += 1;
        if let Some(candidate_observation) = candidate.get(key) {
            accumulator.shared_features += 1;
            accumulator
                .absolute_deltas
                .push((candidate_observation.value - baseline_observation.value).abs());
            if strict_sign_flip(
                baseline_observation.value,
                candidate_observation.value,
                epsilon,
            ) {
                accumulator.strict_sign_flips += 1;
            }
            if let (Some(left), Some(right)) =
                (&baseline_observation.status, &candidate_observation.status)
            {
                if left != right {
                    accumulator.status_disagreements += 1;
                }
            }
        }
    }

    let mut output = Vec::new();
    for (sample_id, mut accumulator) in by_sample {
        accumulator.absolute_deltas.sort_by(f64::total_cmp);
        let union = accumulator.baseline_features + accumulator.candidate_features
            - accumulator.shared_features;
        let mae = if accumulator.absolute_deltas.is_empty() {
            0.0
        } else {
            accumulator.absolute_deltas.iter().sum::<f64>()
                / accumulator.absolute_deltas.len() as f64
        };
        let key_jaccard = if union == 0 {
            1.0
        } else {
            accumulator.shared_features as f64 / union as f64
        };
        if key_jaccard < 1.0
            || mae > 0.0
            || accumulator.strict_sign_flips > 0
            || accumulator.status_disagreements > 0
        {
            output.push(SampleChange {
                sample_id,
                shared_features: accumulator.shared_features,
                key_jaccard,
                mae,
                p95_abs_delta: quantile(&accumulator.absolute_deltas, 0.95),
                strict_sign_flips: accumulator.strict_sign_flips,
                status_disagreements: accumulator.status_disagreements,
            });
        }
    }

    output.sort_by(|left, right| {
        right
            .p95_abs_delta
            .total_cmp(&left.p95_abs_delta)
            .then_with(|| right.mae.total_cmp(&left.mae))
            .then_with(|| left.sample_id.cmp(&right.sample_id))
    });
    output.truncate(limit);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(entries: &[(&str, &str, f64, Option<&str>)]) -> BTreeMap<RecordKey, Observation> {
        entries
            .iter()
            .map(|(sample, feature, value, status)| {
                (
                    RecordKey {
                        sample_id: (*sample).to_owned(),
                        feature_id: (*feature).to_owned(),
                    },
                    Observation {
                        value: *value,
                        status: status.map(str::to_owned),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn interpolates_quantiles() {
        assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.5);
        assert_eq!(quantile(&[], 0.95), 0.0);
    }

    #[test]
    fn pearson_handles_identity_inverse_and_constant() {
        assert_eq!(pearson(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]), Some(1.0));
        assert_eq!(pearson(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]), Some(-1.0));
        assert_eq!(pearson(&[1.0, 1.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn spearman_uses_average_tie_ranks() {
        let correlation = spearman(&[1.0, 1.0, 3.0], &[2.0, 2.0, 9.0]);
        assert_eq!(correlation, Some(1.0));
    }

    #[test]
    fn key_metrics_include_reference_specific_keys() {
        let baseline = records(&[("s1", "shared", 1.0, None), ("s1", "left", 2.0, None)]);
        let candidate = records(&[("s1", "shared", 1.0, None), ("s1", "right", 2.0, None)]);
        let metrics = key_metrics(&baseline, &candidate);
        assert_eq!(metrics.shared, 1);
        assert_eq!(metrics.union, 3);
        assert_eq!(metrics.baseline_only, 1);
        assert_eq!(metrics.candidate_only, 1);
        assert!((metrics.jaccard - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn epsilon_excludes_near_zero_sign_changes() {
        let baseline = records(&[("s1", "near", -0.01, None), ("s1", "far", -2.0, None)]);
        let candidate = records(&[("s1", "near", 0.01, None), ("s1", "far", 3.0, None)]);
        assert_eq!(
            numeric_metrics(&baseline, &candidate, 0.02).strict_sign_flips,
            1
        );
        assert_eq!(
            numeric_metrics(&baseline, &candidate, 0.0).strict_sign_flips,
            2
        );
    }

    #[test]
    fn status_rate_uses_only_pairs_with_two_statuses() {
        let baseline = records(&[
            ("s1", "f1", 1.0, Some("pass")),
            ("s1", "f2", 2.0, Some("pass")),
            ("s1", "f3", 3.0, None),
        ]);
        let candidate = records(&[
            ("s1", "f1", 1.0, Some("fail")),
            ("s1", "f2", 2.0, None),
            ("s1", "f3", 3.0, Some("pass")),
        ]);
        let metrics = status_metrics(&baseline, &candidate);
        assert_eq!(metrics.comparable, 1);
        assert_eq!(metrics.disagreements, 1);
        assert_eq!(metrics.disagreement_rate, Some(1.0));
    }

    #[test]
    fn top_k_ties_have_stable_key_order() {
        let baseline = records(&[
            ("s1", "a", 10.0, None),
            ("s1", "b", 10.0, None),
            ("s1", "c", 1.0, None),
        ]);
        let candidate = records(&[
            ("s1", "a", -10.0, None),
            ("s1", "b", 10.0, None),
            ("s1", "c", 9.0, None),
        ]);
        let metrics = top_k_metrics(&baseline, &candidate, 2);
        assert_eq!(metrics.overlap, 2);
        assert_eq!(metrics.jaccard, 1.0);
    }

    #[test]
    fn largest_changes_use_key_as_tie_breaker() {
        let baseline = records(&[("s2", "f", 0.0, None), ("s1", "f", 0.0, None)]);
        let candidate = records(&[("s2", "f", 1.0, None), ("s1", "f", -1.0, None)]);
        let changes = largest_changes(&baseline, &candidate, 2);
        assert_eq!(changes[0].sample_id, "s1");
        assert_eq!(changes[1].sample_id, "s2");
    }

    #[test]
    fn unchanged_keys_are_not_presented_as_changes() {
        let baseline = records(&[("s1", "f", 1.0, Some("pass"))]);
        let candidate = baseline.clone();
        assert!(largest_changes(&baseline, &candidate, 10).is_empty());
        assert!(most_affected_samples(&baseline, &candidate, 0.0, 10).is_empty());
    }

    #[test]
    fn disjoint_keys_have_defined_set_metrics_and_undefined_correlations() {
        let baseline = records(&[("s1", "left", 1.0, None)]);
        let candidate = records(&[("s1", "right", 2.0, None)]);
        let keys = key_metrics(&baseline, &candidate);
        let numeric = numeric_metrics(&baseline, &candidate, 0.0);
        let top = top_k_metrics(&baseline, &candidate, 5);
        assert_eq!(keys.jaccard, 0.0);
        assert_eq!(numeric.compared, 0);
        assert_eq!(numeric.pearson, None);
        assert_eq!(numeric.spearman, None);
        assert_eq!(top.jaccard, 0.0);
        assert_eq!(
            most_affected_samples(&baseline, &candidate, 0.0, 10).len(),
            1
        );
    }
}
