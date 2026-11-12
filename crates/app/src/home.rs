//! The Home screen: start something new, open a recent picture (as a
//! large thumbnail), get back work that wasn't saved, and see when a
//! newer Duck Quill is out. Shown at start (unless a picture was opened
//! by double-clicking it, or it's switched off) and from File → Home.
//!
//! The thumbnails are made when a picture is saved and kept in the
//! settings folder's `thumbnails` folder, one small PNG per picture.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, Button, ColorImage, RichText, TextureHandle, vec2};
use quill_engine::document::{Background, Document};
use quill_engine::image::Image;

use crate::autosave::{self, LeftBehind};
use crate::theme;

/// A thumbnail's largest size.
pub const THUMB: [u32; 2] = [240, 180];

/// What was clicked on the Home screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeAction {
    New,
    NewAnimation,
    NewSprite,
    Open,
    /// The recent picture at this place in the list.
    OpenRecent(usize),
    /// The unsaved copy at this place in the list.
    Restore(usize),
    ThrowAway(usize),
    /// The update banner's button.
    Update,
    /// Back to the picture (Home was opened from the menu).
    Back,
}

/// The update line at the top, if there's anything to say.
pub struct Banner {
    pub text: String,
    /// The button's words ("Update", "Download"), if it has one.
    pub button: Option<&'static str>,
}

#[derive(Default)]
pub struct Home {
    /// Loaded thumbnails (None: there is none for that picture).
    textures: HashMap<PathBuf, Option<TextureHandle>>,
}

/// Where the thumbnail of the picture at `picture` is kept.
pub fn thumbnail_path(settings_folder: &Path, picture: &Path) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    crate::settings::full_path(picture).hash(&mut hasher);
    settings_folder
        .join("thumbnails")
        .join(format!("{:016x}.png", hasher.finish()))
}

/// Makes the thumbnail of a picture just saved, on its own thread (the
/// picture is copied first; its pixels are shared, so that's quick).
pub fn save_thumbnail(document: &Document, settings_folder: &Path, picture: &Path) {
    let document = document.clone();
    let path = thumbnail_path(settings_folder, picture);
    std::thread::spawn(move || {
        let _ = write_thumbnail(&document, &path);
    });
}

fn write_thumbnail(document: &Document, path: &Path) -> Result<(), String> {
    let first = document.frame_picture(0);
    let image = Image::of_document(&first, Background::Paper).fit_within(THUMB[0], THUMB[1]);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    image.save_png(path, false).map_err(|e| e.to_string())
}

fn load_texture(ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
    let image = Image::load(path).ok()?;
    let pixels: Vec<egui::Color32> = (0..image.height())
        .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, g, b, a] = image.pixel(x, y);
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        })
        .collect();
    let color = ColorImage::new([image.width() as usize, image.height() as usize], pixels);
    Some(ctx.load_texture(
        path.display().to_string(),
        color,
        egui::TextureOptions::LINEAR,
    ))
}

impl Home {
    /// A thumbnail may have changed (the picture was saved again).
    pub fn forget(&mut self, picture: &Path) {
        self.textures.remove(&crate::settings::full_path(picture));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        settings_folder: Option<&Path>,
        recent: &[PathBuf],
        left_behind: &[LeftBehind],
        banner: Option<Banner>,
        show_at_start: &mut bool,
        can_go_back: bool,
    ) -> Option<HomeAction> {
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.heading(RichText::new("Duck Quill").size(28.0).strong());
                ui.weak(env!("CARGO_PKG_VERSION"));
                if let Some(banner) = &banner {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        if let Some(button) = banner.button
                            && ui.add(theme::primary_button(button)).clicked()
                        {
                            chosen = Some(HomeAction::Update);
                        }
                        ui.label(&banner.text);
                    });
                }
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let size = vec2(0.0, 40.0);
                for (label, action) in [
                    ("New picture", HomeAction::New),
                    ("New animation", HomeAction::NewAnimation),
                    ("New pixel sprite or tile", HomeAction::NewSprite),
                    ("Open…", HomeAction::Open),
                ] {
                    if ui.add(Button::new(label).min_size(size)).clicked() {
                        chosen = Some(action);
                    }
                }
                if can_go_back
                    && ui
                        .add(Button::new("Back to the picture").min_size(size))
                        .clicked()
                {
                    chosen = Some(HomeAction::Back);
                }
            });

            let ctx = ui.ctx().clone();
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("Recent").strong().size(16.0));
            });
            ui.add_space(6.0);
            if recent.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.weak("Pictures you open or save show up here.");
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.add_space(16.0);
                for (index, picture) in recent.iter().enumerate() {
                    let texture = self
                        .textures
                        .entry(crate::settings::full_path(picture))
                        .or_insert_with(|| {
                            settings_folder.and_then(|folder| {
                                load_texture(&ctx, &thumbnail_path(folder, picture))
                            })
                        })
                        .clone();
                    let name = picture
                        .file_stem()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    if card(ui, texture.as_ref(), &name, &picture.display().to_string()) {
                        chosen = Some(HomeAction::OpenRecent(index));
                    }
                }
            });

            if !left_behind.is_empty() {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("Not saved last time").strong().size(16.0));
                });
                let now = SystemTime::now();
                for (index, left) in left_behind.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(format!(
                            "\"{}\", kept {}",
                            left.note.name,
                            autosave::how_long_ago(left.note.copied, now)
                        ));
                        if ui.add(theme::primary_button("Restore")).clicked() {
                            chosen = Some(HomeAction::Restore(index));
                        }
                        if ui
                            .button("Throw away")
                            .on_hover_text("Delete the copy for good")
                            .clicked()
                        {
                            chosen = Some(HomeAction::ThrowAway(index));
                        }
                    });
                }
            }
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.checkbox(show_at_start, "Show this at start");
                ui.weak("   Find a command: Ctrl+K");
            });
        });
        chosen
    }
}

/// A recent picture: its thumbnail (or a plain box) and name. True if
/// clicked.
fn card(ui: &mut egui::Ui, texture: Option<&TextureHandle>, name: &str, tip: &str) -> bool {
    let size = vec2(THUMB[0] as f32, THUMB[1] as f32);
    let response = ui
        .vertical(|ui| {
            ui.set_width(size.x);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            let visuals = ui.style().interact(&response);
            ui.painter()
                .rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
            if let Some(texture) = texture {
                let image_size = texture.size_vec2();
                let scale = (size.x / image_size.x).min(size.y / image_size.y).min(1.0);
                let shown = egui::Rect::from_center_size(rect.center(), image_size * scale);
                ui.painter().image(
                    texture.id(),
                    shown,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "🦆",
                    egui::FontId::proportional(40.0),
                    ui.visuals().weak_text_color(),
                );
            }
            ui.painter()
                .rect_stroke(rect, 6.0, visuals.bg_stroke, egui::StrokeKind::Inside);
            ui.add(egui::Label::new(name).truncate());
            response
        })
        .inner;
    response.on_hover_text(tip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_picture_gets_a_small_thumbnail_of_its_first_frame() {
        let folder = std::env::temp_dir().join(format!("quill-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let document = Document::new(800, 600);
        let picture = folder.join("pond.quill");
        let path = thumbnail_path(&folder, &picture);
        write_thumbnail(&document, &path).unwrap();
        let image = Image::load(&path).unwrap();
        assert_eq!(
            (image.width(), image.height()),
            (THUMB[0], THUMB[0] * 3 / 4)
        );
        // Each picture has its own, and the same picture always the same.
        assert_eq!(path, thumbnail_path(&folder, &picture));
        assert_ne!(path, thumbnail_path(&folder, &folder.join("duck.quill")));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
