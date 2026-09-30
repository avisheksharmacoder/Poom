pub mod daemon;
pub mod diff;
pub mod export;
pub mod prune;
pub mod stats;
pub mod studio;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "poom",
    author = "Poom Contributors",
    version,
    about = "Poom: Pure-Rust Observability Studio",
    long_about = "High-performance, local-first LLM & AI agent observability studio built entirely in pure Rust."
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Custom redb database path")]
    pub db: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Custom IPC domain socket path")]
    pub socket: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Enable verbose debug logging")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Launch the native desktop studio and embedded daemon [default]")]
    Studio,

    #[command(about = "Run the headless ingestion daemon and storage engine")]
    Daemon {
        #[arg(long, default_value = "false", help = "Disable background retention pruner")]
        no_prune: bool,
    },

    #[command(about = "Purge historical traces older than retention cutoff")]
    Prune {
        #[arg(short, long, default_value = "14", help = "Retention threshold in days")]
        days: u32,

        #[arg(long, help = "Calculate purgeable records without deleting")]
        dry_run: bool,
    },

    #[command(about = "Export trace hierarchies to JSON or OTLP format")]
    Export {
        #[arg(short, long, help = "Trace ID (UUIDv7) to export")]
        trace: Option<String>,

        #[arg(long, help = "Export traces created within duration in hours (e.g. 24)")]
        hours: Option<u64>,

        #[arg(short, long, default_value = "json", help = "Export format: json or otlp")]
        format: ExportFormat,

        #[arg(short, long, help = "Output destination file path [default: stdout]")]
        output: Option<PathBuf>,
    },

    #[command(about = "Compute visual diff between two spans or prompts")]
    Diff {
        #[arg(long, help = "First Span ID (UUIDv7)")]
        span1: String,

        #[arg(long, help = "Second Span ID (UUIDv7)")]
        span2: String,

        #[arg(long, default_value = "prompt", help = "Field to diff: prompt, completion, or all")]
        field: DiffField,
    },

    #[command(about = "Display summary metrics and database storage stats")]
    Stats,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Otlp,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffField {
    Prompt,
    Completion,
    All,
}
