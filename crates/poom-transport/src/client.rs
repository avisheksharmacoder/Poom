use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::codec::Framed;

use crate::codec::{ClientCodec, ClientFramed};
use crate::error::TransportError;
use crate::path::default_ipc_path;
use crate::stream::IpcStream;

#[cfg(unix)]
use tokio::net::UnixStream;

/// Asynchronous client connector for establishing resilient IPC connections.
pub struct IpcClient;

impl IpcClient {
    /// Connects to the default system IPC path.
    pub async fn connect_default() -> Result<ClientFramed, TransportError> {
        let path = default_ipc_path();
        Self::connect(&path).await
    }

    /// Connects directly to a specific IPC path.
    pub async fn connect(path: impl AsRef<Path>) -> Result<ClientFramed, TransportError> {
        let path = path.as_ref();

        #[cfg(unix)]
        {
            let stream = UnixStream::connect(path).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound || e.kind() == std::io::ErrorKind::ConnectionRefused {
                    TransportError::ConnectionRefused(format!("Daemon not running at {}", path.display()))
                } else {
                    TransportError::Io(e)
                }
            })?;

            let ipc_stream = IpcStream::from_unix(stream);
            Ok(Framed::new(ipc_stream, ClientCodec))
        }

        #[cfg(not(unix))]
        {
            Err(TransportError::InvalidPath("Non-unix IPC currently unsupported".to_string()))
        }
    }

    /// Connects with a bounded connection timeout.
    pub async fn connect_with_timeout(
        path: impl AsRef<Path>,
        timeout: Duration,
    ) -> Result<ClientFramed, TransportError> {
        tokio::time::timeout(timeout, Self::connect(path))
            .await
            .map_err(|_| TransportError::Timeout("Connection timed out".to_string()))?
    }

    /// Connects with exponential backoff retry to handle daemon startup latency.
    pub async fn connect_with_retry(
        path: impl AsRef<Path>,
        max_retries: usize,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Result<ClientFramed, TransportError> {
        let path = path.as_ref();
        let mut backoff = initial_backoff;
        let mut last_err = TransportError::ConnectionRefused("Connection attempts exhausted".to_string());

        for attempt in 0..max_retries {
            match Self::connect(path).await {
                Ok(framed) => return Ok(framed),
                Err(e) => {
                    last_err = e;
                    if attempt + 1 < max_retries {
                        sleep(backoff).await;
                        backoff = std::cmp::min(backoff.saturating_mul(2), max_backoff);
                    }
                }
            }
        }

        Err(last_err)
    }
}
