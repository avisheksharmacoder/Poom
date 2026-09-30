use std::env;
use std::path::PathBuf;

/// Environment variable to explicitly override the IPC socket or named pipe path.
pub const ENV_POOM_SOCKET_PATH: &str = "POOM_SOCKET_PATH";

/// Resolves the default IPC path for the current platform and user session.
///
/// Hierarchy:
/// 1. `POOM_SOCKET_PATH` environment variable if present.
/// 2. Unix: `$XDG_RUNTIME_DIR/poom/poom.sock`.
/// 3. Unix fallback: `/tmp/poom-<uid>.sock` (or `/tmp/poom.sock`).
/// 4. Windows: `\\.\pipe\poom`.
pub fn default_ipc_path() -> PathBuf {
    if let Ok(custom) = env::var(ENV_POOM_SOCKET_PATH) {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom.trim());
        }
    }

    #[cfg(unix)]
    {
        if let Ok(xdg_runtime) = env::var("XDG_RUNTIME_DIR") {
            if !xdg_runtime.trim().is_empty() {
                return PathBuf::from(xdg_runtime).join("poom").join("poom.sock");
            }
        }

        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/poom-{}.sock", uid))
    }

    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\poom")
    }
}
