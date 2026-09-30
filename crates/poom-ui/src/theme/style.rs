use iced::widget::{button, container, text_input};
use iced::{border::Radius, Border, Color, Shadow, Theme};

use super::colors::*;

/// Primary background container style.
pub fn primary_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_primary(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

/// Surface card style with subtle 1px border.
pub fn surface_card(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_surface(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: border_subtle(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        shadow: Shadow::default(),
    }
}

/// Active / selected item card style.
pub fn selected_card(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_selected(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: BORDER_ACCENT,
            width: 1.0,
            radius: Radius::from(6.0),
        },
        shadow: Shadow::default(),
    }
}

/// Header / toolbar container style.
pub fn header_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_surface(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: border_subtle(theme),
            width: 1.0,
            radius: Radius::default(),
        },
        shadow: Shadow::default(),
    }
}

/// Subtle badge pill style for tags and kinds.
pub fn badge_container(bg: Color, border_color: Color) -> impl Fn(&Theme) -> container::Style {
    move |theme: &Theme| container::Style {
        background: Some(bg.into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::from(4.0),
        },
        shadow: Shadow::default(),
    }
}

/// Standard flat navigation button.
pub fn nav_button(theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Active => bg_card(theme),
        button::Status::Hovered => bg_hover(theme),
        button::Status::Pressed => bg_primary(theme),
        button::Status::Disabled => bg_surface(theme),
    };

    button::Style {
        background: Some(bg.into()),
        text_color: text_primary(theme),
        border: Border {
            color: border_subtle(theme),
            width: 1.0,
            radius: Radius::from(4.0),
        },
        shadow: Shadow::default(),
    }
}

/// Primary accent button (e.g., active tab or filter).
pub fn active_nav_button(theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Active => BORDER_ACCENT,
        button::Status::Hovered => KIND_CHAIN,
        button::Status::Pressed => bg_card(theme),
        button::Status::Disabled => bg_surface(theme),
    };

    button::Style {
        background: Some(bg.into()),
        text_color: Color::WHITE,
        border: Border {
            color: BORDER_ACCENT,
            width: 1.0,
            radius: Radius::from(4.0),
        },
        shadow: Shadow::default(),
    }
}

/// Text input styling supporting light and dark themes.
pub fn dark_text_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Active => border_subtle(theme),
        text_input::Status::Hovered => border_focus(theme),
        text_input::Status::Focused => BORDER_ACCENT,
        text_input::Status::Disabled => border_subtle(theme),
    };

    text_input::Style {
        background: bg_card(theme).into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::from(4.0),
        },
        icon: text_muted(theme),
        placeholder: text_muted(theme),
        value: text_primary(theme),
        selection: BORDER_ACCENT,
    }
}

/// Floating modal dialog container style with elevated shadow.
pub fn modal_container(theme: &Theme) -> container::Style {
    let shadow_color = match theme {
        Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.12),
        _ => Color::from_rgba(0.0, 0.0, 0.0, 0.45),
    };

    container::Style {
        background: Some(bg_surface(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: border_subtle(theme),
            width: 1.0,
            radius: Radius::from(8.0),
        },
        shadow: Shadow {
            color: shadow_color,
            offset: iced::Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
    }
}

/// Code snippet / monospace preview container style.
pub fn code_box_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_primary(theme).into()),
        text_color: Some(text_primary(theme)),
        border: Border {
            color: border_subtle(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        shadow: Shadow::default(),
    }
}
