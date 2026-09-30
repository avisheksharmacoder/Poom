use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::signal;

use poom_daemon::{DaemonConfig, PoomDaemon};
use poom_storage::StoragePruner;

use crate::error::CliResult;

pub fn execute_daemon(
    db_path: &Path,
    socket_path: &Path,
    no_prune: bool,
    _verbose: bool,
) -> CliResult<()> {
    println!("\x1b[1;36m====================== Poom Ingestion Daemon ======================\x1b[0m");
    println!("Process ID (PID):   \x1b[37m{}\x1b[0m", std::process::id());
    println!("Database Path:      \x1b[32m{}\x1b[0m", db_path.display());
    println!("IPC Socket Path:    \x1b[34m{}\x1b[0m", socket_path.display());
    println!("Background Pruning: \x1b[37m{}\x1b[0m", if no_prune { "disabled" } else { "enabled (14-day retention, hourly check)" });
    println!("Mode:               \x1b[35mHeadless Daemon (Zero GPU / X11 linkages)\x1b[0m");
    println!("\x1b[90m-------------------------------------------------------------------\x1b[0m");

    // 1. Build bounded Tokio runtime (2 worker threads to keep RSS < 15MB)
    let rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    rt.block_on(async {
        let mut config = DaemonConfig::default();
        config.db_path = db_path.to_path_buf();
        config.socket_path = socket_path.to_path_buf();

        let daemon = PoomDaemon::start(config).await?;
        let daemon_arc = Arc::new(daemon);
        let d = Arc::clone(&daemon_arc);

        // 2. Spawn daemon server loop
        let server_handle = tokio::spawn(async move {
            if let Err(e) = d.run().await {
                eprintln!("\x1b[31m[DAEMON ERROR]: {e}\x1b[0m");
            }
        });

        // 3. Optional periodic background pruner
        let storage_clone = daemon_arc.storage().clone();
        let pruner_handle = if !no_prune {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(3600));
                interval.tick().await; // skip immediate tick

                loop {
                    interval.tick().await;
                    let now_nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0);
                    let cutoff_14d = now_nanos.saturating_sub(14 * 86_400 * 1_000_000_000);

                    let pruner = StoragePruner::new(storage_clone.database());
                    match pruner.prune_older_than(cutoff_14d) {
                        Ok(count) if count > 0 => {
                            println!("\x1b[90m[MAINTENANCE]: Pruned {count} expired traces.\x1b[0m");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("\x1b[33m[MAINTENANCE ERROR]: {e}\x1b[0m");
                        }
                    }
                }
            }))
        } else {
            None
        };

        println!("\x1b[1;32m✓ Daemon is ready and listening for incoming spans.\x1b[0m (Press Ctrl+C to terminate)");

        // 4. Trap shutdown signals (Ctrl+C and SIGTERM)
        wait_for_shutdown_signal().await;

        println!("\n\x1b[1;33mShutting down daemon cleanly... (flushing pending micro-batches)\x1b[0m");

        daemon_arc.shutdown();

        if let Some(h) = pruner_handle {
            h.abort();
        }
        server_handle.abort();

        // Remove socket file on clean shutdown
        if socket_path.exists() {
            let _ = std::fs::remove_file(socket_path);
        }

        println!("\x1b[32m✓ Daemon shutdown complete. Database is safe.\x1b[0m");
        Ok::<(), crate::error::CliError>(())
    })?;

    Ok(())
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
