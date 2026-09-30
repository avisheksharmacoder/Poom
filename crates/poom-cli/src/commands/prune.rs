use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use poom_storage::keys::decode_time_index_key;
use poom_storage::schema::TIME_INDEX_TABLE;
use poom_storage::StoragePruner;

use crate::error::CliResult;

pub fn execute_prune(db_path: &Path, days: u32, dry_run: bool) -> CliResult<()> {
    if !db_path.exists() {
        println!("\x1b[33mDatabase does not exist at: {}\x1b[0m", db_path.display());
        return Ok(());
    }

    let start = Instant::now();
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let nanos_in_day = 86_400_000_000_000u64;
    let retention_nanos = days as u64 * nanos_in_day;
    let cutoff_nanos = now_nanos.saturating_sub(retention_nanos);

    let db = redb::Database::open(db_path)?;

    if dry_run {
        let read_txn = db.begin_read()?;
        let time_index = read_txn.open_table(TIME_INDEX_TABLE)?;

        let mut end_key = [0xFFu8; 24];
        end_key[0..8].copy_from_slice(&cutoff_nanos.to_be_bytes());

        let start_key = [0u8; 24];
        let range = time_index.range::<&[u8; 24]>(&start_key..=&end_key)?;

        let mut purgeable_count = 0;
        for item in range {
            let (k, _) = item?;
            let (timestamp, _) = decode_time_index_key(k.value());
            if timestamp <= cutoff_nanos {
                purgeable_count += 1;
            }
        }

        println!("\x1b[1;33m[DRY RUN]\x1b[0m Retention threshold: {} days", days);
        println!("Cutoff timestamp: {} nanos", cutoff_nanos);
        println!(
            "Traces that would be pruned: \x1b[1;31m{}\x1b[0m",
            purgeable_count
        );
        println!("\x1b[90mScanned in {:?}. No records were deleted.\x1b[0m", start.elapsed());
        return Ok(());
    }

    let pruner = StoragePruner::new(&db);
    let pruned_traces = pruner.prune_older_than(cutoff_nanos)?;

    println!(
        "\x1b[32mSuccessfully pruned {} historical traces older than {} days.\x1b[0m",
        pruned_traces, days
    );
    println!("\x1b[90mCompleted in {:?}\x1b[0m", start.elapsed());

    Ok(())
}
