//! The look of the app: dark or light, and a light touch of the Duck
//! Quill brand colours. The canvas is the star, so almost everything stays
//! the calm grey egui gives us; only the "this one is chosen" colour and
//! the main button of a dialog use the brand.

use eframe::egui::{self, Button, Color32, RichText, Stroke, Theme, ThemePreference};

/// Night indigo: the selected tool and the selected layer.
pub const NIGHT_INDIGO: Color32 = Color32::from_rgb(0x55, 0x46, 0xD0);
/// Duck yellow: the main button of a dialog (Save, OK).
pub const DUCK_YELLOW: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x2E);
/// Ink navy: text on duck yellow, dark enough to read easily.
pub const INK_NAVY: Color32 = Color32::from_rgb(0x1B, 0x14, 0x46);

/// Gives both the dark and the light look the brand's selection colour.
/// Done once at start: egui keeps a style for each theme, and switching
/// themes picks the other one, so both need it.
pub fn apply_brand(ctx: &egui::Context) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            // `selection` is what egui uses for "chosen": selected
            // buttons and list rows, and selected text. White on indigo
            // reads well in both themes.
            style.visuals.selection.bg_fill = NIGHT_INDIGO;
            style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        });
    }
}

/// A dialog's main button: duck yellow with navy text, so it's the
/// obvious one to press.
pub fn primary_button(text: &str) -> Button<'static> {
    Button::new(RichText::new(text).color(INK_NAVY).strong()).fill(DUCK_YELLOW)
}

/// The theme saved in the settings ("dark", "light", "system"); dark if
/// none was saved.
pub fn from_name(name: Option<&str>) -> ThemePreference {
    match name {
        Some("light") => ThemePreference::Light,
        Some("system") => ThemePreference::System,
        _ => ThemePreference::Dark,
    }
}

/// The name a theme is saved under.
pub fn name(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::Dark => "dark",
        ThemePreference::Light => "light",
        ThemePreference::System => "system",
    }
}
