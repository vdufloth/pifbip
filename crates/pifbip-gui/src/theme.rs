//! Dark theme matching the GNOME terminal default grey (#2E3436), with the
//! pifbip brand colors: primary #FF3C00 (orange-red), secondary #00C3FF (cyan).

use iced::widget::{button, svg};
use iced::{Background, Border, Color, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// GNOME Tango default terminal background, rgb(46, 52, 54).
pub const BACKGROUND: Color = rgb(0x2E, 0x34, 0x36);
/// Slightly lighter panel surface for contrast.
pub const SURFACE: Color = rgb(0x3A, 0x41, 0x43);
pub const TEXT: Color = rgb(0xD3, 0xD7, 0xCF);
/// Brand primary — orange-red.
pub const PRIMARY: Color = rgb(0xFF, 0x3C, 0x00);
/// Brand secondary — cyan.
pub const SECONDARY: Color = rgb(0x00, 0xC3, 0xFF);
pub const DANGER: Color = rgb(0xCC, 0x00, 0x00);

/// The custom dark theme used by the whole application.
pub fn theme() -> Theme {
    Theme::custom(
        "pifbip-dark".to_string(),
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: PRIMARY,
            success: SECONDARY,
            danger: DANGER,
        },
    )
}

/// Full-window background fill (#2E3436).
pub fn root_panel(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(BACKGROUND)),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

/// A raised surface panel (left list / preview frame).
pub fn surface_panel(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(TEXT),
        border: Border {
            color: rgb(0x29, 0x2E, 0x2F),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

/// Highlight style for the current file row in the list (brand primary).
pub fn selected_row(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(PRIMARY)),
        text_color: Some(BACKGROUND),
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}

fn flat_button(bg: Color, fg: Color, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color { a: 0.85, ..bg },
        _ => bg,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: fg,
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}

/// A suggestion row: cyan (secondary) when selected, surface otherwise.
pub fn suggestion(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_t, status| {
        if selected {
            flat_button(SECONDARY, BACKGROUND, status)
        } else {
            flat_button(SURFACE, TEXT, status)
        }
    }
}

/// The primary call-to-action button (brand orange).
pub fn primary_button(_t: &Theme, status: button::Status) -> button::Style {
    flat_button(PRIMARY, BACKGROUND, status)
}

/// A neutral surface button (Back / Skip / Browse).
pub fn neutral_button(_t: &Theme, status: button::Status) -> button::Style {
    flat_button(SURFACE, TEXT, status)
}

/// Tint a monochrome icon SVG with `color`.
pub fn icon(color: Color) -> impl Fn(&Theme, svg::Status) -> svg::Style {
    move |_t, _s| svg::Style { color: Some(color) }
}
