//! Dark theme matching the GNOME terminal default grey (#2E3436).

use iced::widget::container;
use iced::{Background, Border, Color, Theme};

/// GNOME Tango default terminal background, rgb(46, 52, 54).
pub const BACKGROUND: Color = Color::from_rgb(
    0x2E as f32 / 255.0,
    0x34 as f32 / 255.0,
    0x36 as f32 / 255.0,
);
/// Slightly lighter panel surface for contrast.
pub const SURFACE: Color = Color::from_rgb(
    0x3A as f32 / 255.0,
    0x41 as f32 / 255.0,
    0x43 as f32 / 255.0,
);
pub const TEXT: Color = Color::from_rgb(
    0xD3 as f32 / 255.0,
    0xD7 as f32 / 255.0,
    0xCF as f32 / 255.0,
);
pub const PRIMARY: Color = Color::from_rgb(
    0x72 as f32 / 255.0,
    0x9F as f32 / 255.0,
    0xCF as f32 / 255.0,
);
pub const SUCCESS: Color = Color::from_rgb(
    0x8A as f32 / 255.0,
    0xE2 as f32 / 255.0,
    0x34 as f32 / 255.0,
);
pub const DANGER: Color = Color::from_rgb(
    0xCC as f32 / 255.0,
    0x00 as f32 / 255.0,
    0x00 as f32 / 255.0,
);

/// The custom dark theme used by the whole application.
pub fn theme() -> Theme {
    Theme::custom(
        "pifbip-dark".to_string(),
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: PRIMARY,
            success: SUCCESS,
            danger: DANGER,
        },
    )
}

/// Full-window background fill (#2E3436).
pub fn root_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BACKGROUND)),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

/// A raised surface panel (left list / preview frame).
pub fn surface_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(TEXT),
        border: Border {
            color: Color::from_rgb(0.16, 0.18, 0.19),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    }
}

/// Highlight style for the currently-selected list/suggestion row.
pub fn selected_row(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PRIMARY)),
        text_color: Some(BACKGROUND),
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}
