use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use refaudit::audit;
use refaudit::input;
use refaudit::model::{FailOn, GateSummary, Outcome, OutputFormat};
use refaudit::render::render;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "refaudit",
    version,
    about = "Audit downstream result stability across reference genomes",
    long_about = "Compare matched sample/feature values produced against multiple reference genomes. Reports quantify instability; they do not identify which reference is biologically correct."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Audit every non-baseline reference against one baseline.
    Audit(AuditArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DelimiterArg {
    Csv,
    Tsv,
}

impl From<DelimiterArg> for input::Delimiter {
    fn from(value: DelimiterArg) -> Self {
        match value {
            DelimiterArg::Csv => Self::Csv,
            DelimiterArg::Tsv => Self::Tsv,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatArg {
    Markdown,
    Json,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Markdown => Self::Markdown,
            FormatArg::Json => Self::Json,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FailOnArg {
    Error,
    Warning,
}

impl From<FailOnArg> for FailOn {
    fn from(value: FailOnArg) -> Self {
        match value {
            FailOnArg::Error => Self::Error,
            FailOnArg::Warning => Self::Warning,
        }
    }
}

#[derive(Debug, Args)]
struct AuditArgs {
    /// Long-format CSV/TSV input. Gzip is detected by file content.
    #[arg(short, long)]
    input: PathBuf,

    /// Reference name to use as the comparison baseline.
    #[arg(short, long)]
    baseline: String,

    /// Override delimiter inference from .csv, .tsv, .tab, and optional .gz.
    #[arg(long, value_enum)]
    delimiter: Option<DelimiterArg>,

    /// Name of the reference column.
    #[arg(long, default_value = "reference")]
    reference_column: String,

    /// Name of the sample identifier column.
    #[arg(long, default_value = "sample_id")]
    sample_column: String,

    /// Name of the feature identifier column.
    #[arg(long, default_value = "feature_id")]
    feature_column: String,

    /// Name of the numeric result column.
    #[arg(long, default_value = "value")]
    value_column: String,

    /// Optional status column name. It is used only when present in the input.
    #[arg(long, default_value = "status")]
    status_column: String,

    /// Values in [-epsilon, +epsilon] are neutral for strict sign-flip checks.
    #[arg(long, default_value_t = 0.0)]
    epsilon: f64,

    /// Number of largest absolute values used for the top-k overlap metric.
    #[arg(long, default_value_t = 100)]
    top_k: usize,

    /// Maximum changed keys and affected samples retained per comparison.
    #[arg(long, default_value_t = 10)]
    max_examples: usize,

    /// Fail if key-set Jaccard is below this value.
    #[arg(long)]
    min_key_jaccard: Option<f64>,

    /// Fail if Spearman correlation is below this value.
    #[arg(long)]
    min_spearman: Option<f64>,

    /// Fail if the strict sign-flip rate is above this value.
    #[arg(long)]
    max_sign_flip_rate: Option<f64>,

    /// Fail if the status-disagreement rate is above this value.
    #[arg(long)]
    max_status_disagreement_rate: Option<f64>,

    /// Fail if the p95 absolute numeric delta is above this value.
    #[arg(long)]
    max_p95_abs_delta: Option<f64>,

    /// Report serialization format.
    #[arg(long, value_enum, default_value_t = FormatArg::Markdown)]
    format: FormatArg,

    /// Write the report to a file instead of standard output.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Process exit 2 on gate errors only, or on any warning.
    #[arg(long, value_enum, default_value_t = FailOnArg::Error)]
    fail_on: FailOnArg,
}

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Input(#[from] input::InputError),
    #[error(transparent)]
    Audit(#[from] audit::AuditError),
    #[error("cannot serialize report: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("cannot write report to {path}: {source}")]
    WriteFile { path: PathBuf, source: io::Error },
    #[error("cannot write report to standard output: {0}")]
    WriteStdout(io::Error),
}

fn execute(args: AuditArgs) -> Result<ExitCode, CliError> {
    let columns = input::ColumnNames {
        reference: args.reference_column,
        sample: args.sample_column,
        feature: args.feature_column,
        value: args.value_column,
        status: args.status_column,
    };
    let dataset = input::read_dataset(&args.input, args.delimiter.map(Into::into), &columns)?;
    let config = audit::AuditConfig {
        baseline: args.baseline,
        epsilon: args.epsilon,
        top_k: args.top_k,
        max_examples: args.max_examples,
        gates: GateSummary {
            min_key_jaccard: args.min_key_jaccard,
            min_spearman: args.min_spearman,
            max_sign_flip_rate: args.max_sign_flip_rate,
            max_status_disagreement_rate: args.max_status_disagreement_rate,
            max_p95_abs_delta: args.max_p95_abs_delta,
        },
    };
    let report = audit::run_audit(&dataset, &args.input, &config)?;
    let output = render(&report, args.format.into())?;

    if let Some(path) = args.output {
        fs::write(&path, output).map_err(|source| CliError::WriteFile { path, source })?;
    } else {
        io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(CliError::WriteStdout)?;
    }

    let fail_on: FailOn = args.fail_on.into();
    let failed = report.outcome == Outcome::Fail
        || (fail_on == FailOn::Warning && report.outcome == Outcome::Warn);
    Ok(if failed {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Audit(args) => execute(args),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}
