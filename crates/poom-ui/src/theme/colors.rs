use iced::Color;
use poom_types::SpanKind;

// Base Surface & Background Colors
pub const BG_PRIMARY: Color = Color::from_rgb(0.043, 0.059, 0.098);    // #0B0F19
pub const BG_SURFACE: Color = Color::from_rgb(0.067, 0.094, 0.153);    // #111827
pub const BG_CARD: Color = Color::from_rgb(0.118, 0.161, 0.231);       // #1E293B
pub const BG_HOVER: Color = Color::from_rgb(0.200, 0.255, 0.333);      // #334155
pub const BG_SELECTED: Color = Color::from_rgb(0.150, 0.200, 0.300);   // Darker accent highlight

// Border Colors
pub const BORDER_SUBTLE: Color = Color::from_rgb(0.122, 0.161, 0.216); // #1F2937
pub const BORDER_FOCUS: Color = Color::from_rgb(0.278, 0.333, 0.412);  // #475569
pub const BORDER_ACCENT: Color = Color::from_rgb(0.384, 0.514, 0.980); // #6283FA

// Text Colors
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.973, 0.980, 0.988);   // #F8FAFC
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.580, 0.639, 0.722); // #94A3B8
pub const TEXT_MUTED: Color = Color::from_rgb(0.392, 0.455, 0.545);     // #64748B

// Semantic Outcome Colors
pub const ACCENT_SUCCESS: Color = Color::from_rgb(0.063, 0.725, 0.506); // Emerald #10B981
pub const ACCENT_ERROR: Color = Color::from_rgb(0.937, 0.267, 0.267);   // Crimson #EF4444
pub const ACCENT_WARNING: Color = Color::from_rgb(0.961, 0.620, 0.043); // Amber #F59E0B
pub const ACCENT_INFO: Color = Color::from_rgb(0.231, 0.510, 0.965);    // Blue #3B82F6

// SpanKind Specific Colors
pub const KIND_AGENT: Color = Color::from_rgb(0.545, 0.361, 0.965);     // Deep Purple #8B5CF6
pub const KIND_CHAIN: Color = Color::from_rgb(0.231, 0.510, 0.965);     // Slate Blue #3B82F6
pub const KIND_LLM: Color = Color::from_rgb(0.063, 0.725, 0.506);       // Emerald #10B981
pub const KIND_TOOL: Color = Color::from_rgb(0.961, 0.620, 0.043);      // Amber #F59E0B
pub const KIND_FUNCTION: Color = Color::from_rgb(0.392, 0.455, 0.545);  // Slate #64748B
pub const KIND_HTTP: Color = Color::from_rgb(0.024, 0.714, 0.831);      // Cyan #06B6D4

// Light Mode Base Surface & Background Colors
pub const LIGHT_BG_PRIMARY: Color = Color::from_rgb(0.965, 0.973, 0.984);  // #F6F8FA
pub const LIGHT_BG_SURFACE: Color = Color::from_rgb(1.0, 1.0, 1.0);        // #FFFFFF
pub const LIGHT_BG_CARD: Color = Color::from_rgb(0.941, 0.953, 0.969);     // #F0F3F7
pub const LIGHT_BG_HOVER: Color = Color::from_rgb(0.898, 0.918, 0.941);    // #E5EAEF
pub const LIGHT_BG_SELECTED: Color = Color::from_rgb(0.855, 0.898, 0.957); // Soft blue highlight

// Light Mode Border Colors
pub const LIGHT_BORDER_SUBTLE: Color = Color::from_rgb(0.867, 0.894, 0.925); // #DDE4EC
pub const LIGHT_BORDER_FOCUS: Color = Color::from_rgb(0.600, 0.650, 0.720);
pub const LIGHT_BORDER_ACCENT: Color = Color::from_rgb(0.231, 0.510, 0.965);

// Light Mode Text Colors
pub const LIGHT_TEXT_PRIMARY: Color = Color::from_rgb(0.059, 0.090, 0.165);   // #0F172A
pub const LIGHT_TEXT_SECONDARY: Color = Color::from_rgb(0.278, 0.333, 0.412); // #475569
pub const LIGHT_TEXT_MUTED: Color = Color::from_rgb(0.392, 0.455, 0.545);     // #64748B

/// Resolves background primary color based on active Theme.
pub fn bg_primary(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BG_PRIMARY,
        _ => BG_PRIMARY,
    }
}

/// Resolves surface container color based on active Theme.
pub fn bg_surface(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BG_SURFACE,
        _ => BG_SURFACE,
    }
}

/// Resolves card container color based on active Theme.
pub fn bg_card(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BG_CARD,
        _ => BG_CARD,
    }
}

/// Resolves hover highlight color based on active Theme.
pub fn bg_hover(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BG_HOVER,
        _ => BG_HOVER,
    }
}

/// Resolves selected item color based on active Theme.
pub fn bg_selected(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BG_SELECTED,
        _ => BG_SELECTED,
    }
}

/// Resolves subtle border color based on active Theme.
pub fn border_subtle(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BORDER_SUBTLE,
        _ => BORDER_SUBTLE,
    }
}

/// Resolves focused border color based on active Theme.
pub fn border_focus(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_BORDER_FOCUS,
        _ => BORDER_FOCUS,
    }
}

/// Resolves primary text color based on active Theme.
pub fn text_primary(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_TEXT_PRIMARY,
        _ => TEXT_PRIMARY,
    }
}

/// Resolves secondary text color based on active Theme.
pub fn text_secondary(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_TEXT_SECONDARY,
        _ => TEXT_SECONDARY,
    }
}

/// Resolves muted text color based on active Theme.
pub fn text_muted(theme: &iced::Theme) -> Color {
    match theme {
        iced::Theme::Light => LIGHT_TEXT_MUTED,
        _ => TEXT_MUTED,
    }
}

/// Returns the primary theme color for a specific SpanKind.
pub fn color_for_kind(kind: SpanKind) -> Color {
    match kind {
        SpanKind::Agent => KIND_AGENT,
        SpanKind::Chain => KIND_CHAIN,
        SpanKind::Llm => KIND_LLM,
        SpanKind::Tool => KIND_TOOL,
        SpanKind::Function => KIND_FUNCTION,
        SpanKind::Http => KIND_HTTP,
    }
}

/// Returns a subtle, low-opacity background tint for a SpanKind badge.
pub fn bg_tint_for_kind(kind: SpanKind) -> Color {
    let mut c = color_for_kind(kind);
    c.a = 0.18;
    c
}
