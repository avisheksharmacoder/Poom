pub mod app;
pub mod error;
pub mod state;
pub mod subscription;
pub mod theme;
pub mod views;
pub mod widgets;

use std::path::Path;
use poom_daemon::EventBroadcaster;
use poom_storage::StorageEngine;

pub use app::AppState;
pub use error::UiError;
pub use state::Message;

/// Launches the Poom Observability Studio with an existing storage handle and optional live broadcaster.
pub fn run_studio(
    storage: StorageEngine,
    db_path: impl Into<String>,
    broadcaster: Option<EventBroadcaster>,
) -> iced::Result {
    let db_path_str = db_path.into();

    #[cfg(unix)]
    {
        if let Ok(lang) = std::env::var("LANG") {
            if !lang.to_lowercase().contains("utf") {
                std::env::set_var("LANG", format!("{lang}.UTF-8"));
            }
        } else {
            std::env::set_var("LANG", "en_US.UTF-8");
        }
        std::env::set_var("LC_ALL", "C.UTF-8");
        std::env::set_var("LC_CTYPE", "C.UTF-8");
    }

    iced::application(
        "Poom — Observability Studio",
        AppState::update,
        AppState::view,
    )
    .theme(AppState::theme)
    .scale_factor(AppState::scale_factor)
    .subscription(AppState::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1440.0, 900.0),
        min_size: Some(iced::Size::new(960.0, 600.0)),
        position: iced::window::Position::Centered,
        ..Default::default()
    })
    .run_with(move || AppState::new(storage, db_path_str, broadcaster))
}

/// Standalone launcher opening an embedded redb storage directly from a filesystem path.
pub fn run_standalone(db_path: impl AsRef<Path>) -> Result<(), UiError> {
    let path = db_path.as_ref();
    let storage = StorageEngine::open(path)?;
    let path_str = path.to_string_lossy().to_string();

    run_studio(storage, path_str, None).map_err(|e| UiError::Iced(e.to_string()))
}
