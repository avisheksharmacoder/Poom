use std::path::{Path, PathBuf};
use crate::error::TransportError;
use crate::stream::IpcStream;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

/// Platform-agnostic IPC server listener.
///
/// Automatically implements stale socket probe detection, secure POSIX permission
/// hardening (mode `0600`), and RAII socket cleanup on drop.
pub struct IpcListener {
    #[cfg(unix)]
    inner: UnixListener,
    path: PathBuf,
    clean_on_drop: bool,
}

impl IpcListener {
    /// Binds to the specified IPC path with automated stale socket self-healing.
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();

        #[cfg(unix)]
        {
            if path.exists() {
                // Perform a non-blocking connect probe to detect an active daemon
                match UnixStream::connect(&path).await {
                    Ok(_) => {
                        return Err(TransportError::AddressInUse(path));
                    }
                    Err(_) => {
                        // The socket file exists but refuses connection: it's stale.
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }

            // Ensure parent directory exists with secure 0700 permissions
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                    let perms = std::fs::Permissions::from_mode(0o700);
                    let _ = std::fs::set_permissions(parent, perms);
                }
            }

            let listener = UnixListener::bind(&path)?;

            // Restrict socket file permissions to 0600 (owner read/write only)
            let socket_perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, socket_perms);

            Ok(Self {
                inner: listener,
                path,
                clean_on_drop: true,
            })
        }

        #[cfg(not(unix))]
        {
            Err(TransportError::InvalidPath("Non-unix IPC currently unsupported".to_string()))
        }
    }

    /// Accepts an incoming client IPC connection.
    pub async fn accept(&self) -> Result<IpcStream, TransportError> {
        #[cfg(unix)]
        {
            let (stream, _addr) = self.inner.accept().await?;
            Ok(IpcStream::from_unix(stream))
        }

        #[cfg(not(unix))]
        {
            Err(TransportError::Disconnected)
        }
    }

    /// Returns a reference to the bound IPC path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Configures whether the socket file should be deleted when the listener is dropped.
    pub fn set_clean_on_drop(&mut self, clean: bool) {
        self.clean_on_drop = clean;
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        if self.clean_on_drop {
            #[cfg(unix)]
            {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
