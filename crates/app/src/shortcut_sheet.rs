//! Help → Keyboard shortcuts (or the ? key): every shortcut on one sheet.

use eframe::egui::{self, Key, RichText};

use crate::commands::{self, Keys, Menu};

/// What isn't a command but is good to know: dragging, and keys that
/// work only in the middle of something. (The commands' keys come from
/// the list in `commands.rs`.)
const GESTURES: &[(Group, &str, &str)] = &[
    (Group::Selecting, "Shift + drag", "Add to the selection"),
    (
        Group::Selecting,
        "Alt + drag",
        "Take away from the selection",
    ),
    (
        Group::Selecting,
        "Enter",
        "Close a polygon; put down a transform",
    ),
    (Group::Selecting, "Backspace", "Take back a polygon corner"),
    (
        Group::Selecting,
        "Esc",
        "Give up a polygon; put a transform back",
    ),
    (Group::View, "Wheel", "Zoom"),
    (
        Group::View,
        "Space + drag",
        "Move around (or the middle mouse button)",
    ),
    (Group::View, "Shift + Space + drag", "Turn the canvas"),
];

/// The sheet's groups, in two columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Group {
    Tools,
    View,
    Selecting,
    Edit,
    File,
    Animation,
}

const LEFT: [Group; 2] = [Group::Tools, Group::View];
const RIGHT: [Group; 4] = [Group::Selecting, Group::Edit, Group::File, Group::Animation];

fn group_of(menu: Menu) -> Option<Group> {
    Some(match menu {
        Menu::Tools => Group::Tools,
        Menu::Canvas | Menu::Help | Menu::View => Group::View,
        Menu::Select => Group::Selecting,
        Menu::Edit => Group::Edit,
        Menu::File => Group::File,
        Menu::Animation => Group::Animation,
        _ => return None,
    })
}

fn title(group: Group) -> &'static str {
    match group {
        Group::Tools => "Tools",
        Group::View => "View",
        Group::Selecting => "Selecting",
        Group::Edit => "Edit",
        Group::File => "File",
        Group::Animation => "Animation (with the animation tools on)",
    }
}

/// The lines of one group: (keys, what they do).
fn lines(ctx: &egui::Context, group: Group) -> Vec<(String, String)> {
    let mut lines: Vec<(String, String)> = commands::all()
        .iter()
        .filter(|e| e.keys != Keys::None && group_of(e.menu) == Some(group))
        .map(|e| {
            let what = if e.tip.is_empty() || e.tip.len() > 40 {
                e.name.trim_end_matches('…').to_owned()
            } else {
                let mut tip = e.tip.chars();
                let first = tip.next().map(|c| c.to_lowercase().to_string());
                format!(
                    "{} ({}{})",
                    e.name.trim_end_matches('…'),
                    first.unwrap_or_default(),
                    tip.as_str()
                )
            };
            (e.keys_text(ctx), what)
        })
        .collect();
    lines.extend(
        GESTURES
            .iter()
            .filter(|(g, _, _)| *g == group)
            .map(|(_, keys, what)| ((*keys).to_owned(), (*what).to_owned())),
    );
    lines
}

#[derive(Default)]
pub struct ShortcutSheet {
    pub open: bool,
}

impl ShortcutSheet {
    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Keyboard shortcuts")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    for (column, groups) in [&LEFT[..], &RIGHT[..]].into_iter().enumerate() {
                        let column = &mut columns[column];
                        for group in groups {
                            column.label(RichText::new(title(*group)).strong());
                            egui::Grid::new(title(*group))
                                .num_columns(2)
                                .spacing([12.0, 3.0])
                                .show(column, |ui| {
                                    for (keys, what) in lines(ui.ctx(), *group) {
                                        ui.label(RichText::new(keys).monospace());
                                        ui.label(what);
                                        ui.end_row();
                                    }
                                });
                            column.add_space(8.0);
                        }
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.open = false;
                }
            });
        self.open &= open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_lists_every_tool_key_and_file_shortcut_once() {
        let ctx = egui::Context::default();
        let all: Vec<String> = LEFT
            .iter()
            .chain(RIGHT.iter())
            .flat_map(|group| lines(&ctx, *group).into_iter().map(|(keys, _)| keys))
            .collect();
        for key in [
            "B", "E", "I", "F", "G", "T", "M", "L", "Ctrl+Z", "Ctrl+S", "Ctrl+V", "Ctrl+T", "?",
            "Ctrl+K", ",", ".", "/", "0", "Esc",
        ] {
            assert_eq!(
                all.iter().filter(|k| **k == key).count(),
                1,
                "{key}: {all:?}"
            );
        }
        // Every command with keys is on the sheet.
        let with_keys = commands::all()
            .iter()
            .filter(|e| e.keys != Keys::None && e.menu != Menu::Settings)
            .count();
        assert!(all.len() >= with_keys + GESTURES.len());
    }
}
