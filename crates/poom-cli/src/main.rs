use std::path::PathBuf;
use clap::Parser;

use poom_cli::commands::{Cli, Commands};
use poom_cli::error::CliResult;

fn main() -> CliResult<()> {
    let cli = Cli::parse();

    // 1. Resolve default ~/.poom directory
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let default_poom_dir = PathBuf::from(home).join(".poom");
    let _ = std::fs::create_dir_all(&default_poom_dir);

    // 2. Resolve database path
    let db_path = cli
        .db
        .or_else(|| std::env::var("POOM_DB_PATH").ok().map(PathBuf::from))
        .unwrap_or_else(|| default_poom_dir.join("data.redb"));

    // 3. Resolve socket path
    let socket_path = cli
        .socket
        .or_else(|| std::env::var("POOM_SOCKET_PATH").ok().map(PathBuf::from))
        .unwrap_or_else(poom_transport::default_ipc_path);

    // Ensure parent directory for socket exists
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 4. Command dispatch
    match cli.command {
        None | Some(Commands::Studio) => {
            poom_cli::commands::studio::execute_studio(&db_path, &socket_path)
        }
        Some(Commands::Daemon { no_prune }) => {
            poom_cli::commands::daemon::execute_daemon(&db_path, &socket_path, no_prune, cli.verbose)
        }
        Some(Commands::Prune { days, dry_run }) => {
            poom_cli::commands::prune::execute_prune(&db_path, days, dry_run)
        }
        Some(Commands::Export {
            trace,
            hours,
            format,
            output,
        }) => poom_cli::commands::export::execute_export(&db_path, trace, hours, format, output),
        Some(Commands::Diff {
            span1,
            span2,
            field,
        }) => poom_cli::commands::diff::execute_diff(&db_path, &span1, &span2, field),
        Some(Commands::Stats) => poom_cli::commands::stats::execute_stats(&db_path),
    }
}
