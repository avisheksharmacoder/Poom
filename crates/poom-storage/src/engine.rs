use std::path::{Path, PathBuf};
use std::sync::Arc;
use redb::Database;

use crate::error::StorageError;
use crate::schema::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Core storage engine wrapping redb ACID B-Tree database.
#[derive(Clone)]
pub struct StorageEngine {
    db: Arc<Database>,
    path: PathBuf,
}

impl StorageEngine {
    /// Opens or creates a Poom redb database at the specified file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
                #[cfg(unix)]
                {
                    let perms = std::fs::Permissions::from_mode(0o700);
                    let _ = std::fs::set_permissions(parent, perms);
                }
            }
        }

        let db = Database::create(&path)?;
        Self::initialize_tables(&db)?;

        #[cfg(unix)]
        {
            if path.exists() {
                let perms = std::fs::Permissions::from_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }

        Ok(Self {
            db: Arc::new(db),
            path,
        })
    }

    /// Initializes all database tables in a single write transaction so they exist for readers.
    fn initialize_tables(db: &Database) -> Result<(), StorageError> {
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(SPANS_TABLE)?;
            let _ = write_txn.open_table(TIME_INDEX_TABLE)?;
            let _ = write_txn.open_multimap_table(TRACE_SPANS_TABLE)?;
            let _ = write_txn.open_multimap_table(SPAN_CHILDREN_TABLE)?;
            let _ = write_txn.open_table(TAG_INDEX_TABLE)?;
            let _ = write_txn.open_table(EVALUATIONS_TABLE)?;
            let _ = write_txn.open_multimap_table(TRACE_EVALUATIONS_TABLE)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Returns a reference to the underlying redb Database handle.
    #[inline]
    pub fn database(&self) -> &Arc<Database> {
        &self.db
    }

    /// Returns the database file path.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
