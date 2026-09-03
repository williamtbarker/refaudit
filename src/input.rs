use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;
use thiserror::Error;

use crate::model::{Dataset, Observation, RecordKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delimiter {
    Csv,
    Tsv,
}

impl Delimiter {
    fn byte(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv => b'\t',
        }
    }
}

#[derive(Clone, Debug)]
pub struct ColumnNames {
    pub reference: String,
    pub sample: String,
    pub feature: String,
    pub value: String,
    pub status: String,
}

impl Default for ColumnNames {
    fn default() -> Self {
        Self {
            reference: "reference".to_owned(),
            sample: "sample_id".to_owned(),
            feature: "feature_id".to_owned(),
            value: "value".to_owned(),
            status: "status".to_owned(),
        }
    }
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("cannot open input {path}: {source}")]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot read input {path}: {source}")]
    Read { path: PathBuf, source: csv::Error },
    #[error("required column {column:?} is missing")]
    MissingColumn { column: String },
    #[error("input header {column:?} is duplicated at columns {first_column} and {second_column}")]
    DuplicateHeader {
        column: String,
        first_column: usize,
        second_column: usize,
    },
    #[error("configured columns must have distinct names")]
    DuplicateColumnNames,
    #[error("row {row}: column {column:?} must not be empty")]
    EmptyField { row: usize, column: String },
    #[error("row {row}: value {value:?} is not a finite number")]
    InvalidValue { row: usize, value: String },
    #[error(
        "row {row}: duplicate key for reference {reference:?}, sample {sample:?}, feature {feature:?} (first seen on row {first_row})"
    )]
    DuplicateKey {
        row: usize,
        first_row: usize,
        reference: String,
        sample: String,
        feature: String,
    },
    #[error("input contains no data rows")]
    EmptyInput,
    #[error("input must contain at least two references; found {count}")]
    TooFewReferences { count: usize },
}

pub fn infer_delimiter(path: &Path) -> Delimiter {
    let mut name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(stripped) = name.strip_suffix(".gz") {
        name = stripped.to_owned();
    }
    if name.ends_with(".tsv") || name.ends_with(".tab") {
        Delimiter::Tsv
    } else {
        Delimiter::Csv
    }
}

fn open_reader(path: &Path) -> Result<Box<dyn Read>, InputError> {
    let probe = File::open(path).map_err(|source| InputError::Open {
        path: path.to_owned(),
        source,
    })?;
    let mut probe = BufReader::new(probe);
    let mut magic = [0_u8; 2];
    let bytes = probe.read(&mut magic).map_err(|source| InputError::Open {
        path: path.to_owned(),
        source,
    })?;

    let file = File::open(path).map_err(|source| InputError::Open {
        path: path.to_owned(),
        source,
    })?;
    if bytes == 2 && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(BufReader::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

fn required_index(headers: &csv::StringRecord, column: &str) -> Result<usize, InputError> {
    headers
        .iter()
        .position(|header| header == column)
        .ok_or_else(|| InputError::MissingColumn {
            column: column.to_owned(),
        })
}

fn nonempty<'a>(
    record: &'a csv::StringRecord,
    index: usize,
    row: usize,
    column: &str,
) -> Result<&'a str, InputError> {
    let value = record.get(index).unwrap_or_default().trim();
    if value.is_empty() {
        Err(InputError::EmptyField {
            row,
            column: column.to_owned(),
        })
    } else {
        Ok(value)
    }
}

pub fn read_dataset(
    path: &Path,
    delimiter: Option<Delimiter>,
    columns: &ColumnNames,
) -> Result<Dataset, InputError> {
    let configured = [
        columns.reference.as_str(),
        columns.sample.as_str(),
        columns.feature.as_str(),
        columns.value.as_str(),
        columns.status.as_str(),
    ];
    if configured.iter().collect::<BTreeSet<_>>().len() != configured.len() {
        return Err(InputError::DuplicateColumnNames);
    }

    let reader = open_reader(path)?;
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(delimiter.unwrap_or_else(|| infer_delimiter(path)).byte())
        .trim(csv::Trim::All)
        .flexible(false)
        .from_reader(reader);

    let headers = csv
        .headers()
        .map_err(|source| InputError::Read {
            path: path.to_owned(),
            source,
        })?
        .clone();
    let mut header_positions = BTreeMap::new();
    for (index, header) in headers.iter().enumerate() {
        if let Some(first_index) = header_positions.insert(header, index) {
            return Err(InputError::DuplicateHeader {
                column: header.to_owned(),
                first_column: first_index + 1,
                second_column: index + 1,
            });
        }
    }
    let reference_index = required_index(&headers, &columns.reference)?;
    let sample_index = required_index(&headers, &columns.sample)?;
    let feature_index = required_index(&headers, &columns.feature)?;
    let value_index = required_index(&headers, &columns.value)?;
    let status_index = headers
        .iter()
        .position(|header| header == columns.status.as_str());

    let mut references = BTreeMap::<String, BTreeMap<RecordKey, Observation>>::new();
    let mut first_rows = BTreeMap::<(String, RecordKey), usize>::new();
    let mut samples = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut record_count = 0;

    for (offset, result) in csv.records().enumerate() {
        let row = offset + 2;
        let record = result.map_err(|source| InputError::Read {
            path: path.to_owned(),
            source,
        })?;
        let reference = nonempty(&record, reference_index, row, &columns.reference)?.to_owned();
        let sample = nonempty(&record, sample_index, row, &columns.sample)?.to_owned();
        let feature = nonempty(&record, feature_index, row, &columns.feature)?.to_owned();
        let raw_value = nonempty(&record, value_index, row, &columns.value)?;
        let value = raw_value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| InputError::InvalidValue {
                row,
                value: raw_value.to_owned(),
            })?;
        let status = status_index
            .and_then(|index| record.get(index))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);

        let key = RecordKey {
            sample_id: sample.clone(),
            feature_id: feature.clone(),
        };
        let seen_key = (reference.clone(), key.clone());
        if let Some(first_row) = first_rows.insert(seen_key, row) {
            return Err(InputError::DuplicateKey {
                row,
                first_row,
                reference,
                sample,
                feature,
            });
        }

        references
            .entry(reference)
            .or_default()
            .insert(key, Observation { value, status });
        samples.insert(sample);
        features.insert(feature);
        record_count += 1;
    }

    if record_count == 0 {
        return Err(InputError::EmptyInput);
    }
    if references.len() < 2 {
        return Err(InputError::TooFewReferences {
            count: references.len(),
        });
    }

    Ok(Dataset {
        references,
        record_count,
        samples: samples.into_iter().collect(),
        features: features.into_iter().collect(),
        has_status: status_index.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_columns_match_documented_schema() {
        let columns = ColumnNames::default();
        assert_eq!(columns.reference, "reference");
        assert_eq!(columns.sample, "sample_id");
        assert_eq!(columns.feature, "feature_id");
        assert_eq!(columns.value, "value");
        assert_eq!(columns.status, "status");
    }

    #[test]
    fn infers_tsv_before_gzip_suffix() {
        assert_eq!(infer_delimiter(Path::new("results.tsv.gz")), Delimiter::Tsv);
        assert_eq!(infer_delimiter(Path::new("results.csv.gz")), Delimiter::Csv);
    }
}
