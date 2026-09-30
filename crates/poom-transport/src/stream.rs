use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(unix)]
use tokio::net::UnixStream;

/// Platform-agnostic asynchronous bi-directional IPC stream.
pub struct IpcStream {
    #[cfg(unix)]
    pub(crate) inner: UnixStream,
}

impl IpcStream {
    #[cfg(unix)]
    pub fn from_unix(stream: UnixStream) -> Self {
        Self { inner: stream }
    }

    #[cfg(unix)]
    pub fn into_unix(self) -> UnixStream {
        self.inner
    }

    #[cfg(unix)]
    pub fn peer_pid(&self) -> Option<u32> {
        self.inner.peer_cred().ok().and_then(|c| c.pid().map(|p| p as u32))
    }

    #[cfg(not(unix))]
    pub fn peer_pid(&self) -> Option<u32> {
        None
    }
}

impl AsyncRead for IpcStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        #[cfg(unix)]
        {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
        #[cfg(not(unix))]
        {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::Unsupported, "Non-unix IPC unsupported")))
        }
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        #[cfg(unix)]
        {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }
        #[cfg(not(unix))]
        {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::Unsupported, "Non-unix IPC unsupported")))
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        #[cfg(unix)]
        {
            Pin::new(&mut self.inner).poll_flush(cx)
        }
        #[cfg(not(unix))]
        {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        #[cfg(unix)]
        {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
        #[cfg(not(unix))]
        {
            Poll::Ready(Ok(()))
        }
    }
}
