use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn stable_input_passes_and_renders_markdown() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args(["--baseline", "hg38", "--top-k", "3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: PASS**"))
        .stdout(predicate::str::contains("chm13 vs baseline"))
        .stdout(predicate::str::contains("Largest shared-key changes").not())
        .stdout(predicate::str::contains("Most affected samples").not());
}

#[test]
fn differences_warn_without_failing_by_default() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("reference_sensitive.tsv"))
        .args(["--baseline", "hg38"])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: WARN**"))
        .stdout(predicate::str::contains("STRICT_SIGN_FLIPS"))
        .stdout(predicate::str::contains("KEY_SET_DIFFERENCE"));
}

#[test]
fn numeric_only_change_is_not_mislabeled_as_pass() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("changed.csv");
    fs::write(
        &path,
        "reference,sample_id,feature_id,value\na,s1,f1,1.0\na,s1,f2,2.0\nb,s1,f1,1.01\nb,s1,f2,2.0\n",
    )
    .unwrap();

    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: WARN**"))
        .stdout(predicate::str::contains("NUMERIC_VALUES_DIFFER"));
}

#[test]
fn fail_on_warning_returns_exit_two_after_writing_report() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("reference_sensitive.tsv"))
        .args(["--baseline", "hg38", "--fail-on", "warning"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("**Outcome: WARN**"));
}

#[test]
fn gate_failure_is_machine_readable_and_returns_exit_two() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    let assertion = command
        .args(["audit", "--input"])
        .arg(fixture("reference_sensitive.tsv"))
        .args([
            "--baseline",
            "hg38",
            "--format",
            "json",
            "--min-key-jaccard",
            "0.95",
            "--max-sign-flip-rate",
            "0.05",
            "--min-spearman",
            "0.90",
            "--max-status-disagreement-rate",
            "0.10",
        ])
        .assert()
        .code(2);
    let value: serde_json::Value = serde_json::from_slice(&assertion.get_output().stdout).unwrap();
    assert_eq!(value["outcome"], "FAIL");
    assert!(value["findings"].as_array().unwrap().iter().any(|finding| {
        finding["code"] == "KEY_JACCARD_BELOW_MIN" || finding["code"] == "SIGN_FLIP_RATE_ABOVE_MAX"
    }));
}

#[test]
fn every_gate_can_pass_at_its_boundary() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args([
            "--baseline",
            "hg38",
            "--min-key-jaccard",
            "1",
            "--min-spearman",
            "1",
            "--max-sign-flip-rate",
            "0",
            "--max-status-disagreement-rate",
            "0",
            "--max-p95-abs-delta",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: PASS**"));
}

#[test]
fn duplicate_keys_are_rejected_with_both_row_numbers() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("invalid_duplicate.tsv"))
        .args(["--baseline", "hg38"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("duplicate key"))
        .stderr(predicate::str::contains("first seen on row 2"));
}

#[test]
fn unknown_baseline_lists_available_references() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args(["--baseline", "grch37"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "available references: chm13, hg38",
        ));
}

#[test]
fn gzip_is_detected_from_magic_bytes_not_extension() {
    let directory = tempfile::tempdir().unwrap();
    let source = fs::read(fixture("stable.tsv")).unwrap();
    let path = directory.path().join("compressed.tsv");
    let compression = flate2::Compression::default();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), compression);
    encoder.write_all(&source).unwrap();
    fs::write(&path, encoder.finish().unwrap()).unwrap();

    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "hg38"])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: PASS**"));
}

#[test]
fn custom_columns_and_csv_override_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("data.unknown");
    fs::write(
        &path,
        "genome,sample,feature,score,call\na,s1,f1,1.0,yes\na,s1,f2,2.0,no\nb,s1,f1,1.0,YES\nb,s1,f2,2.0,NO\n",
    )
    .unwrap();

    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args([
            "--baseline",
            "a",
            "--delimiter",
            "csv",
            "--reference-column",
            "genome",
            "--sample-column",
            "sample",
            "--feature-column",
            "feature",
            "--value-column",
            "score",
            "--status-column",
            "call",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Outcome: PASS**"));
}

#[test]
fn invalid_gate_range_is_a_configuration_error() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args(["--baseline", "hg38", "--min-spearman", "1.1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("must be between 0 and 1"));
}

#[test]
fn invalid_non_ratio_settings_are_configuration_errors() {
    for (flag, value, expected) in [
        ("--epsilon", "-1", "--epsilon must be"),
        ("--top-k", "0", "--top-k must be greater than 0"),
        (
            "--max-examples",
            "0",
            "--max-examples must be greater than 0",
        ),
        ("--max-p95-abs-delta", "-1", "--max-p95-abs-delta must be"),
    ] {
        let mut command = Command::cargo_bin("refaudit").unwrap();
        command
            .args(["audit", "--input"])
            .arg(fixture("stable.tsv"))
            .args(["--baseline", "hg38"])
            .arg(format!("{flag}={value}"))
            .assert()
            .code(1)
            .stderr(predicate::str::contains(expected));
    }
}

#[test]
fn output_file_contains_complete_report() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("report.json");
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args(["--baseline", "hg38", "--format", "json", "--output"])
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    let value: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(value["schema_version"], "1.0.0");
}

#[test]
fn non_finite_values_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nonfinite.csv");
    fs::write(
        &path,
        "reference,sample_id,feature_id,value\na,s1,f1,NaN\nb,s1,f1,1.0\n",
    )
    .unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a finite number"));
}

#[test]
fn missing_required_column_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.csv");
    fs::write(&path, "reference,sample_id,value\na,s1,1\nb,s1,1\n").unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "required column \"feature_id\" is missing",
        ));
}

#[test]
fn duplicate_headers_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("headers.csv");
    fs::write(
        &path,
        "reference,sample_id,feature_id,value,value\na,s1,f1,1,1\nb,s1,f1,1,1\n",
    )
    .unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "header \"value\" is duplicated at columns 4 and 5",
        ));
}

#[test]
fn duplicate_configured_column_names_are_rejected() {
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("stable.tsv"))
        .args(["--baseline", "hg38", "--feature-column", "sample_id"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "configured columns must have distinct names",
        ));
}

#[test]
fn empty_required_field_is_rejected_with_row_and_column() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("empty-field.csv");
    fs::write(
        &path,
        "reference,sample_id,feature_id,value\na,,f1,1\nb,s1,f1,1\n",
    )
    .unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "row 2: column \"sample_id\" must not be empty",
        ));
}

#[test]
fn header_only_input_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("empty.csv");
    fs::write(&path, "reference,sample_id,feature_id,value\n").unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("input contains no data rows"));
}

#[test]
fn missing_input_file_is_reported_without_panic() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("absent.tsv");
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot open input"));
}

#[test]
fn one_reference_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("one.csv");
    fs::write(&path, "reference,sample_id,feature_id,value\na,s1,f1,1\n").unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("at least two references"));
}

#[test]
fn undefined_status_gate_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("no-status.csv");
    fs::write(
        &path,
        "reference,sample_id,feature_id,value\na,s1,f1,1\nb,s1,f1,1\n",
    )
    .unwrap();
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(path)
        .args(["--baseline", "a", "--max-status-disagreement-rate", "0"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains(
            "STATUS_DISAGREEMENT_RATE_UNDEFINED",
        ));
}

#[test]
fn reports_are_byte_deterministic_for_same_command() {
    let run = || {
        Command::cargo_bin("refaudit")
            .unwrap()
            .args(["audit", "--input"])
            .arg(fixture("reference_sensitive.tsv"))
            .args([
                "--baseline",
                "hg38",
                "--format",
                "json",
                "--max-examples",
                "4",
            ])
            .output()
            .unwrap()
            .stdout
    };
    assert_eq!(run(), run());
}

#[test]
fn report_file_is_written_even_when_gate_fails() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("failed.json");
    let mut command = Command::cargo_bin("refaudit").unwrap();
    command
        .args(["audit", "--input"])
        .arg(fixture("reference_sensitive.tsv"))
        .args([
            "--baseline",
            "hg38",
            "--max-p95-abs-delta",
            "0.01",
            "--format",
            "json",
            "--output",
        ])
        .arg(&output)
        .assert()
        .code(2);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(value["outcome"], "FAIL");
}
