//! The timeline: the frames of a hand-drawn animation, at the bottom of
//! the window while the animation tools are on (View → Animation tools).
//!
//! A little picture of every frame (click to go there, drag to reorder,
//! right-click to duplicate, delete or hold), play and pause, fps, loop,
//! the onion skin (frames before in red and after in blue, faint, under
//! the drawing), and the named stretches ("idle", "run") under the
//! frames. Everything it changes is asked of the app as a
//! [`TimelineAction`], so it goes through the undo history.
//!
//! The frames' little pictures and the onion skin are worked out on
//! background threads (one each, so the onion skin never waits for the
//! pictures) from a copy of the picture (cheap: the copy shares every
//! tile), so drawing and stepping never wait for them. A frame's picture
//! is made again only when what it shows changed: each frame has a
//! fingerprint of its layers' settings and tiles.

use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{
    self, Button, Color32, ColorImage, RichText, TextureHandle, TextureOptions, vec2,
};
use quill_engine::document::{Background, Document};
use quill_engine::frames::{MAX_HOLD, Stretch, Timeline, TimelineSettings};
use quill_engine::image::Image;

/// The onion skin's colours: before in red, after in blue (as before).
pub const ONION_BEFORE: [u8; 3] = [230, 60, 60];
pub const ONION_AFTER: [u8; 3] = [60, 110, 230];

/// A frame's little picture fits in this box (screen points).
const THUMB: [u32; 2] = [72, 54];

/// What the timeline asks the app to do (each an undo step, except going
/// to a frame).
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineAction {
    GoTo(usize),
    /// An empty frame after the one shown.
    Add,
    /// A copy of the frame shown, after it.
    Duplicate,
    /// A copy of this frame, after it.
    DuplicateOf(usize),
    Delete(usize),
    /// Move frame `.0` to place `.1`.
    Move(usize, usize),
    Hold(usize, u32),
    Timing(TimelineSettings),
}

/// The onion skin's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Onion {
    pub on: bool,
    /// How many frames before and after (0 to 3).
    pub before: usize,
    pub after: usize,
    /// How strong the nearest ones are (0 to 1).
    pub strength: f32,
}

impl Default for Onion {
    fn default() -> Self {
        Self {
            on: false,
            before: 1,
            after: 1,
            strength: 0.35,
        }
    }
}

/// Playing: which frame comes next, and when.
#[derive(Default)]
pub struct Playback {
    pub playing: bool,
    /// Time on the frame shown so far, in seconds.
    elapsed: f32,
}

impl Playback {
    /// Lets `seconds` pass on frame `frame` of `timeline`. Returns the
    /// frame to show now, if it changes. Plays within the frame's
    /// stretch (or the whole timeline) at its fps, each frame for its
    /// hold; stops on the last frame unless it loops.
    pub fn tick(&mut self, timeline: &Timeline, frame: usize, seconds: f32) -> Option<usize> {
        if !self.playing {
            return None;
        }
        let (start, end, fps, looping) = timeline.playing_range(frame);
        if end <= start + 1 {
            return None;
        }
        self.elapsed += seconds.clamp(0.0, 0.25);
        let mut shown = frame;
        loop {
            let hold = timeline.frames().get(shown).map_or(1, |f| f.hold);
            let lasts = hold as f32 / fps.max(1.0);
            if self.elapsed < lasts {
                break;
            }
            self.elapsed -= lasts;
            if shown + 1 < end {
                shown += 1;
            } else if looping {
                shown = start;
            } else {
                self.playing = false;
                self.elapsed = 0.0;
                break;
            }
        }
        (shown != frame).then_some(shown)
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing;
        self.elapsed = 0.0;
    }
}

/// The frames shown faintly behind frame `frame`: (frame, colour,
/// strength), the nearest strongest. Stays in the frame's stretch,
/// going round if it loops.
pub fn onion_frames(timeline: &Timeline, frame: usize, onion: Onion) -> Vec<(usize, [u8; 3], f32)> {
    let (start, end, _, looping) = timeline.playing_range(frame);
    let span = (end - start) as i64;
    let mut found = Vec::new();
    for (count, sign, color) in [
        (onion.before, -1i64, ONION_BEFORE),
        (onion.after, 1, ONION_AFTER),
    ] {
        for k in 1..=count as i64 {
            let n = frame as i64 + sign * k;
            let n = if (start as i64..end as i64).contains(&n) {
                n
            } else if looping && span > 1 {
                start as i64 + (n - start as i64).rem_euclid(span)
            } else {
                continue;
            };
            if n as usize == frame
                || found
                    .iter()
                    .any(|(f, _, _): &(usize, _, _)| *f == n as usize)
            {
                continue;
            }
            // Each step further away is fainter.
            let strength = onion.strength * (1.0 - 0.3 * (k - 1) as f32);
            found.push((n as usize, color, strength.max(0.05)));
        }
    }
    found
}

/// The onion skin as one see-through picture the canvas's size: only
/// what's drawn frame by frame (the animated layers), never the still
/// ones, which are the same on every frame anyway.
pub fn onion_skin(document: &Document, frames: &[(usize, [u8; 3], f32)]) -> Image {
    let mut skin = Image::new(document.width(), document.height(), [0, 0, 0, 0]);
    // Farthest first, so the nearest lie on top.
    for &(frame, color, strength) in frames.iter().rev() {
        let mut picture = document.frame_picture(frame);
        let still: Vec<_> = picture
            .layers()
            .iter()
            .filter(|l| !l.animated())
            .map(|l| l.id())
            .collect();
        for id in still {
            if let Some(layer) = picture.layer_mut(id) {
                layer.set_visible(false);
            }
        }
        let image = Image::of_document(&picture, Background::Transparent);
        skin.paste(&image.tinted(color, strength), 0, 0);
    }
    skin
}

/// Work done on a background thread.
enum Done {
    /// Little pictures: (frame, its fingerprint, the picture).
    Thumbs(Vec<(usize, u64, ColorImage)>),
    Onion(OnionKey, Image),
}

/// What frame `frame` shows, as one number: the paper, the size, and
/// each layer's settings and the tiles of its drawing on that frame (a
/// tile changed by painting is a new tile, so a new number). Cheap: no
/// pixels are looked at.
pub fn fingerprint(document: &Document, frame: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (document.width(), document.height()).hash(&mut hasher);
    let paper = document.paper();
    (paper.r, paper.g, paper.b).hash(&mut hasher);
    let current = document.frame();
    for layer in document.layers() {
        layer.id().hash(&mut hasher);
        (
            layer.visible(),
            layer.opacity().to_bits(),
            layer.blend().name(),
            layer.clip(),
        )
            .hash(&mut hasher);
        let mut tiles: Vec<(i32, i32, usize)> = layer
            .drawing(frame, current)
            .tiles_shared()
            .map(|(c, tile)| (c.x, c.y, std::sync::Arc::as_ptr(tile) as usize))
            .collect();
        tiles.sort_unstable();
        tiles.hash(&mut hasher);
    }
    hasher.finish()
}

/// What an onion skin was made for: the frame, what the frames shown in
/// it hold (their fingerprints, so drawing on the frame shown doesn't
/// make it again), and the settings.
#[derive(Clone, Copy, PartialEq)]
struct OnionKey {
    frame: usize,
    prints: u64,
    before: usize,
    after: usize,
    strength_percent: u32,
}

pub struct TimelineView {
    pub playback: Playback,
    pub onion: Onion,
    /// Each frame's little picture, and the fingerprint of what it shows.
    thumbs: Vec<Option<(u64, TextureHandle)>>,
    /// The frames' fingerprints, for the picture generation `checked`.
    fingerprints: Vec<u64>,
    checked: u64,
    /// The background work: the frames' pictures, and the onion skin.
    thumb_work: Option<Receiver<Done>>,
    onion_work: Option<Receiver<Done>>,
    /// The onion skin shown, and what it was made for.
    onion_made: Option<OnionKey>,
    onion_image: Option<Image>,
    onion_changed: bool,
    /// The Stretches window is open.
    stretches_open: bool,
    /// Its rows while being edited.
    stretches_edit: Vec<Stretch>,
    /// The fps while its number is being dragged.
    fps_edit: Option<f32>,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            playback: Playback::default(),
            onion: Onion::default(),
            thumbs: Vec::new(),
            fingerprints: Vec::new(),
            checked: u64::MAX,
            thumb_work: None,
            onion_work: None,
            onion_made: None,
            onion_image: None,
            onion_changed: false,
            stretches_open: false,
            stretches_edit: Vec::new(),
            fps_edit: None,
        }
    }
}

impl TimelineView {
    /// Forgets everything shown (a new picture was opened).
    pub fn reset(&mut self) {
        let onion = self.onion;
        *self = Self::default();
        self.onion = onion;
    }

    /// Keeps the frames' pictures and the onion skin up to date, in the
    /// background. `generation` changes whenever the picture does; `busy`
    /// (a stroke being painted) waits.
    pub fn refresh(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        generation: u64,
        busy: bool,
    ) {
        // Take in finished work. (A worker that stopped without an answer
        // is let go, so the next one can start.)
        let small = document.width().max(document.height()) <= 256;
        for done in [take(&mut self.thumb_work), take(&mut self.onion_work)]
            .into_iter()
            .flatten()
        {
            match done {
                Done::Thumbs(images) => {
                    for (frame, print, image) in images {
                        if let Some(slot) = self.thumbs.get_mut(frame) {
                            // Pixel sprites stay crisp in their little pictures.
                            let filter = if small {
                                TextureOptions::NEAREST
                            } else {
                                TextureOptions::LINEAR
                            };
                            let texture = ctx.load_texture(format!("frame-{frame}"), image, filter);
                            *slot = Some((print, texture));
                        }
                    }
                }
                Done::Onion(key, image) => {
                    if self.wanted_onion(document, generation) == Some(key) {
                        self.onion_image = Some(image);
                        self.onion_made = Some(key);
                        self.onion_changed = true;
                    }
                }
            }
        }
        let count = document.frame_count();
        if self.thumbs.len() != count {
            self.thumbs.resize_with(count, || None);
            self.thumbs.truncate(count);
        }
        if self.thumb_work.is_some() || self.onion_work.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        if busy || self.playback.playing {
            return;
        }
        // The onion skin: it's on the canvas.
        if self.onion_work.is_none()
            && let Some(key) = self.wanted_onion(document, generation)
            && self.onion_made != Some(key)
        {
            let copy = document.clone();
            let frames = onion_frames(document.timeline(), document.frame(), self.onion);
            let (sender, receiver) = channel();
            std::thread::spawn(move || {
                let _ = sender.send(Done::Onion(key, onion_skin(&copy, &frames)));
            });
            self.onion_work = Some(receiver);
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        // The frames' pictures: those whose fingerprint changed, the one
        // shown first, a few at a time.
        if self.thumb_work.is_some() {
            return;
        }
        if self.checked != generation || self.fingerprints.len() != count {
            self.fingerprints = (0..count).map(|f| fingerprint(document, f)).collect();
            self.checked = generation;
        }
        let current = document.frame();
        let mut stale: Vec<usize> = (0..count)
            .filter(|&f| {
                self.thumbs[f]
                    .as_ref()
                    .is_none_or(|(print, _)| *print != self.fingerprints[f])
            })
            .collect();
        if stale.is_empty() {
            return;
        }
        stale.sort_by_key(|&f| (f != current, f.abs_diff(current)));
        stale.truncate(4);
        let work: Vec<(usize, u64)> = stale
            .into_iter()
            .map(|f| (f, self.fingerprints[f]))
            .collect();
        let copy = document.clone();
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let images = work
                .into_iter()
                .map(|(frame, print)| (frame, print, thumbnail(&copy, frame)))
                .collect();
            let _ = sender.send(Done::Thumbs(images));
        });
        self.thumb_work = Some(receiver);
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    fn wanted_onion(&self, document: &Document, _generation: u64) -> Option<OnionKey> {
        if !(self.onion.on && !self.playback.playing && document.frame_count() > 1) {
            return None;
        }
        let frames = onion_frames(document.timeline(), document.frame(), self.onion);
        let prints = frames.iter().fold(0u64, |acc, (f, _, _)| {
            acc.rotate_left(7) ^ fingerprint(document, *f) ^ *f as u64
        });
        Some(OnionKey {
            frame: document.frame(),
            prints,
            before: self.onion.before,
            after: self.onion.after,
            strength_percent: (self.onion.strength * 100.0).round() as u32,
        })
    }

    /// The onion skin to show on the canvas, when it changed: `Some(None)`
    /// takes it away.
    pub fn take_overlay(&mut self, document: &Document, generation: u64) -> Option<Option<Image>> {
        let wanted = self.wanted_onion(document, generation);
        // Off, or made for another frame (or picture): take it away now
        // rather than show the wrong ghosts while the new one is made.
        if self.onion_made.is_some() && self.onion_made != wanted {
            self.onion_made = None;
            self.onion_image = None;
            return Some(None);
        }
        if std::mem::take(&mut self.onion_changed) {
            return Some(self.onion_image.clone());
        }
        None
    }

    /// The panel. `locked`: frames from a script, not drawn on yet.
    pub fn panel(&mut self, ui: &mut egui::Ui, document: &Document) -> Option<TimelineAction> {
        let mut action = None;
        let timeline = document.timeline();
        let current = document.frame();
        let count = document.frame_count();
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let size = vec2(34.0, 26.0);
            if ui
                .add(Button::new("|◀").min_size(size))
                .on_hover_text("The frame before (,)")
                .clicked()
            {
                self.playback.playing = false;
                action = Some(TimelineAction::GoTo(step(timeline, current, -1)));
            }
            let label = if self.playback.playing {
                "Pause"
            } else {
                "▶ Play"
            };
            if ui
                .add(Button::new(label).min_size(vec2(64.0, 26.0)))
                .on_hover_text("Play or pause (/)")
                .clicked()
            {
                self.playback.toggle();
            }
            if ui
                .add(Button::new("▶|").min_size(size))
                .on_hover_text("The frame after (.)")
                .clicked()
            {
                self.playback.playing = false;
                action = Some(TimelineAction::GoTo(step(timeline, current, 1)));
            }
            ui.separator();
            // fps and loop: the stretch's, or the whole timeline's.
            let mut settings = timeline.settings();
            let stretch = settings
                .stretches
                .iter()
                .position(|s| (s.start..s.end).contains(&current));
            let (mut fps, mut looping) = match stretch {
                Some(i) => (settings.stretches[i].fps, settings.stretches[i].looping),
                None => (settings.fps, settings.looping),
            };
            // While the number is dragged it changes here only; letting go
            // makes it one undo step.
            if let Some(editing) = self.fps_edit {
                fps = editing;
            }
            let response = ui
                .add(
                    egui::DragValue::new(&mut fps)
                        .range(1.0..=60.0)
                        .speed(0.2)
                        .suffix(" fps"),
                )
                .on_hover_text("Frames per second");
            let fps_changed = if response.dragged() {
                self.fps_edit = Some(fps);
                false
            } else if response.drag_stopped() {
                self.fps_edit = None;
                true
            } else {
                self.fps_edit = None;
                response.changed()
            };
            let loop_changed = ui
                .checkbox(&mut looping, "Loop")
                .on_hover_text("Start again after the last frame")
                .changed();
            if fps_changed || loop_changed {
                match stretch {
                    Some(i) => {
                        settings.stretches[i].fps = fps.round();
                        settings.stretches[i].looping = looping;
                    }
                    None => {
                        settings.fps = fps.round();
                        settings.looping = looping;
                    }
                }
                action = Some(TimelineAction::Timing(settings));
            }
            ui.separator();
            ui.checkbox(&mut self.onion.on, "Onion skin")
                .on_hover_text("Frames before (red) and after (blue), faint, under this one");
            ui.add_enabled_ui(self.onion.on, |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.onion.before)
                        .range(0..=3)
                        .prefix("before "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.onion.after)
                        .range(0..=3)
                        .prefix("after "),
                );
                ui.add(
                    egui::Slider::new(&mut self.onion.strength, 0.1..=0.8)
                        .show_value(false)
                        .text("strength"),
                );
            });
            ui.separator();
            if ui
                .button("Stretches…")
                .on_hover_text("Name runs of frames (idle, run...), each with its own speed")
                .clicked()
            {
                self.stretches_edit = timeline.stretches.clone();
                self.stretches_open = true;
            }
            ui.weak(format!("Frame {} of {count}", current + 1));
        });
        ui.add_space(2.0);

        // The frames.
        let mut dropped: Option<(usize, usize)> = None;
        egui::ScrollArea::horizontal()
            .id_salt("frames")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for frame in 0..count {
                        let response = self.frame_cell(ui, timeline, frame, frame == current);
                        if response.clicked() {
                            self.playback.playing = false;
                            action = Some(TimelineAction::GoTo(frame));
                        }
                        response.dnd_set_drag_payload(frame);
                        // Dropped on the left half: before this frame; on
                        // the right half: after it. The line shows which.
                        let after = ui
                            .ctx()
                            .pointer_interact_pos()
                            .is_some_and(|p| p.x > response.rect.center().x);
                        if let Some(from) = response.dnd_release_payload::<usize>() {
                            let to = drop_place(*from, frame, after);
                            if to != *from {
                                dropped = Some((*from, to));
                            }
                        }
                        if response.dnd_hover_payload::<usize>().is_some() {
                            let x = if after {
                                response.rect.right() + 2.0
                            } else {
                                response.rect.left() - 2.0
                            };
                            ui.painter().vline(
                                x,
                                response.rect.y_range(),
                                egui::Stroke::new(2.0, crate::theme::NIGHT_INDIGO),
                            );
                        }
                        response.context_menu(|ui| {
                            if ui.button("Duplicate").clicked() {
                                action = Some(TimelineAction::DuplicateOf(frame));
                                ui.close();
                            }
                            if ui.add_enabled(count > 1, Button::new("Delete")).clicked() {
                                action = Some(TimelineAction::Delete(frame));
                                ui.close();
                            }
                            ui.separator();
                            ui.label("Show it for");
                            let hold = timeline.frames()[frame].hold;
                            for h in [1, 2, 3, 4, 6, 8] {
                                let text = if h == 1 {
                                    "1 frame".into()
                                } else {
                                    format!("{h} frames")
                                };
                                if ui.selectable_label(hold == h, text).clicked() {
                                    action = Some(TimelineAction::Hold(frame, h.min(MAX_HOLD)));
                                    ui.close();
                                }
                            }
                        });
                    }
                    ui.vertical(|ui| {
                        if ui
                            .add(Button::new("+ Frame").min_size(vec2(70.0, 24.0)))
                            .on_hover_text("An empty frame after this one")
                            .clicked()
                        {
                            action = Some(TimelineAction::Add);
                        }
                        if ui
                            .add(Button::new("Duplicate").min_size(vec2(70.0, 24.0)))
                            .on_hover_text("A copy of this frame after it, to change a little")
                            .clicked()
                        {
                            action = Some(TimelineAction::Duplicate);
                        }
                        if ui
                            .add_enabled(
                                count > 1,
                                Button::new("Delete").min_size(vec2(70.0, 24.0)),
                            )
                            .on_hover_text("Delete this frame")
                            .clicked()
                        {
                            action = Some(TimelineAction::Delete(current));
                        }
                    });
                });
                // The stretches under their frames.
                if !timeline.stretches.is_empty() {
                    let cell = THUMB[0] as f32 + ui.spacing().item_spacing.x + 8.0;
                    let (rect, _) = ui
                        .allocate_exact_size(vec2(cell * count as f32, 16.0), egui::Sense::hover());
                    for s in &timeline.stretches {
                        let x0 = rect.left() + cell * s.start as f32 + 2.0;
                        let x1 = rect.left() + cell * s.end as f32 - 6.0;
                        let band = egui::Rect::from_min_max(
                            egui::pos2(x0, rect.top() + 2.0),
                            egui::pos2(x1, rect.bottom()),
                        );
                        ui.painter().rect_filled(
                            band,
                            3.0,
                            crate::theme::NIGHT_INDIGO.gamma_multiply(0.6),
                        );
                        ui.painter().text(
                            band.left_center() + vec2(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{} · {} fps", s.name, s.fps),
                            egui::FontId::proportional(11.0),
                            Color32::WHITE,
                        );
                    }
                }
            });
        if let Some((from, to)) = dropped {
            action = Some(TimelineAction::Move(from, to));
        }
        if let Some(settings) = self.stretches_window(ui.ctx(), document) {
            action = Some(TimelineAction::Timing(settings));
        }
        action
    }

    /// One frame's cell: its picture, number, and hold.
    fn frame_cell(
        &self,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        frame: usize,
        shown: bool,
    ) -> egui::Response {
        let size = vec2(THUMB[0] as f32 + 8.0, THUMB[1] as f32 + 22.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let visuals = ui.visuals();
        let stroke = if shown {
            egui::Stroke::new(2.5, crate::theme::NIGHT_INDIGO)
        } else if response.hovered() {
            egui::Stroke::new(1.0, visuals.widgets.hovered.fg_stroke.color)
        } else {
            egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
        };
        let picture = egui::Rect::from_min_size(
            rect.min + vec2(4.0, 4.0),
            vec2(THUMB[0] as f32, THUMB[1] as f32),
        );
        ui.painter()
            .rect_filled(picture, 2.0, Color32::from_gray(235));
        if let Some(Some((_, texture))) = self.thumbs.get(frame) {
            let [w, h] = texture.size();
            let scale = (THUMB[0] as f32 / w as f32).min(THUMB[1] as f32 / h as f32);
            let fitted = egui::Rect::from_center_size(
                picture.center(),
                vec2(w as f32 * scale, h as f32 * scale),
            );
            ui.painter().image(
                texture.id(),
                fitted,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter()
            .rect_stroke(rect.shrink(1.0), 4.0, stroke, egui::StrokeKind::Inside);
        let hold = timeline.frames()[frame].hold;
        let label = if hold > 1 {
            format!("{} ×{hold}", frame + 1)
        } else {
            format!("{}", frame + 1)
        };
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 9.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            if shown {
                visuals.strong_text_color()
            } else {
                visuals.text_color()
            },
        );
        response.on_hover_text("Click to go to this frame; drag to move it; right-click for more")
    }

    /// The Stretches window: name runs of frames, each with its fps and loop.
    fn stretches_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> Option<TimelineSettings> {
        if !self.stretches_open {
            return None;
        }
        let count = document.frame_count();
        let mut result = None;
        let mut open = true;
        egui::Window::new("Stretches")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("A stretch is one animation in this picture: idle, run, jump.")
                        .weak(),
                );
                ui.label(
                    RichText::new("Each becomes a row of the sprite sheet, with its own speed.")
                        .weak(),
                );
                ui.add_space(6.0);
                let mut remove = None;
                egui::Grid::new("stretch_rows")
                    .num_columns(6)
                    .spacing(vec2(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("From frame");
                        ui.strong("To frame");
                        ui.strong("fps");
                        ui.strong("Loop");
                        ui.end_row();
                        for (i, s) in self.stretches_edit.iter_mut().enumerate() {
                            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(90.0));
                            let mut first = s.start + 1;
                            let mut last = s.end;
                            ui.add(egui::DragValue::new(&mut first).range(1..=count));
                            ui.add(egui::DragValue::new(&mut last).range(first..=count));
                            s.start = first - 1;
                            s.end = last.max(first);
                            ui.add(egui::DragValue::new(&mut s.fps).range(1.0..=60.0));
                            ui.checkbox(&mut s.looping, "");
                            if ui
                                .small_button("✕")
                                .on_hover_text("Remove this stretch")
                                .clicked()
                            {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                if let Some(i) = remove {
                    self.stretches_edit.remove(i);
                }
                if ui.button("+ Add a stretch").clicked() {
                    let start = self
                        .stretches_edit
                        .iter()
                        .map(|s| s.end)
                        .max()
                        .unwrap_or(0)
                        .min(count - 1);
                    self.stretches_edit.push(Stretch {
                        name: format!("animation {}", self.stretches_edit.len() + 1),
                        start,
                        end: count,
                        fps: document.timeline().fps,
                        looping: true,
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add(crate::theme::primary_button("OK")).clicked() {
                        let mut settings = document.timeline().settings();
                        settings.stretches = self.stretches_edit.clone();
                        result = Some(settings);
                        self.stretches_open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.stretches_open = false;
                    }
                });
            });
        self.stretches_open &= open;
        result
    }
}

/// Where a frame dragged from `from` and dropped before (or `after`)
/// frame `target` ends up, counted once it's been taken out.
pub fn drop_place(from: usize, target: usize, after: bool) -> usize {
    let place = target + usize::from(after);
    if place > from { place - 1 } else { place }
}

/// A finished worker's answer, if any: the worker is let go once it has
/// answered, or if it stopped without answering.
fn take(work: &mut Option<Receiver<Done>>) -> Option<Done> {
    let receiver = work.as_ref()?;
    match receiver.try_recv() {
        Ok(done) => {
            *work = None;
            Some(done)
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            *work = None;
            None
        }
    }
}

/// The frame `by` steps from `frame`.
pub fn step(timeline: &Timeline, frame: usize, by: i64) -> usize {
    // Through every frame, round from the last to the first: stepping
    // goes from one stretch into the next (playing stays inside one).
    let count = timeline.frames().len().max(1) as i64;
    (frame as i64 + by).rem_euclid(count) as usize
}

/// A frame's little picture, on the paper.
fn thumbnail(document: &Document, frame: usize) -> ColorImage {
    let picture = Image::of_document(&document.frame_picture(frame), Background::Paper);
    let small = picture.fit_within(THUMB[0] * 2, THUMB[1] * 2);
    let pixels: Vec<Color32> = (0..small.height())
        .flat_map(|y| (0..small.width()).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, g, b, a] = small.pixel(x, y);
            Color32::from_rgba_unmultiplied(r, g, b, a)
        })
        .collect();
    ColorImage::new([small.width() as usize, small.height() as usize], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quill_engine::frames::{FrameCommand, TimelineCommand};

    fn timeline(frames: usize) -> Document {
        let mut document = Document::new(20, 20);
        for _ in 1..frames {
            let end = document.frame_count();
            FrameCommand::add(&mut document, end).unwrap();
        }
        document
    }

    #[test]
    fn playing_follows_the_fps_and_the_holds_and_stops_unless_it_loops() {
        let mut document = timeline(3);
        FrameCommand::hold(&mut document, 1, 2).unwrap();
        let mut settings = document.timeline().settings();
        settings.fps = 10.0;
        settings.looping = false;
        TimelineCommand::apply(&mut document, settings).unwrap();
        let t = document.timeline().clone();
        let mut playback = Playback {
            playing: true,
            elapsed: 0.0,
        };
        assert_eq!(
            playback.tick(&t, 0, 0.05),
            None,
            "a frame is 0.1 s at 10 fps"
        );
        assert_eq!(playback.tick(&t, 0, 0.06), Some(1));
        assert_eq!(playback.tick(&t, 1, 0.15), None, "held twice as long");
        assert_eq!(playback.tick(&t, 1, 0.06), Some(2));
        assert_eq!(playback.tick(&t, 2, 0.2), None, "the end, no loop");
        assert!(!playback.playing);
    }

    #[test]
    fn playing_stays_inside_the_stretch_and_loops_round_it() {
        let mut document = timeline(6);
        let mut settings = document.timeline().settings();
        settings.stretches = vec![
            Stretch {
                name: "idle".into(),
                start: 0,
                end: 2,
                fps: 10.0,
                looping: true,
            },
            Stretch {
                name: "run".into(),
                start: 2,
                end: 6,
                fps: 20.0,
                looping: true,
            },
        ];
        TimelineCommand::apply(&mut document, settings).unwrap();
        let t = document.timeline().clone();
        let mut playback = Playback {
            playing: true,
            elapsed: 0.0,
        };
        assert_eq!(playback.tick(&t, 1, 0.11), Some(0), "round idle");
        assert_eq!(
            playback.tick(&t, 5, 0.051),
            Some(2),
            "round run, at run's fps"
        );
        // Stepping goes through every frame, from one stretch into the next.
        assert_eq!(step(&t, 1, 1), 2);
        assert_eq!(step(&t, 5, 1), 0);
        assert_eq!(step(&t, 0, -1), 5);
        // Dropping: before or after the target, wherever it came from.
        assert_eq!(drop_place(0, 3, false), 2, "before frame 3, once 0 is out");
        assert_eq!(drop_place(0, 3, true), 3);
        assert_eq!(drop_place(4, 1, false), 1);
        assert_eq!(drop_place(4, 1, true), 2);
        assert_eq!(drop_place(2, 2, true), 2, "on itself: no move");
    }

    #[test]
    fn the_onion_skin_shows_neighbours_in_two_tints_fainter_further_away() {
        let document = timeline(5);
        let onion = Onion {
            on: true,
            before: 2,
            after: 1,
            strength: 0.4,
        };
        let frames = onion_frames(document.timeline(), 0, onion);
        // It loops: before frame 0 come 4 and 3; after it, 1.
        assert_eq!(frames.iter().map(|f| f.0).collect::<Vec<_>>(), [4, 3, 1]);
        assert_eq!(frames[0].1, ONION_BEFORE);
        assert_eq!(frames[2].1, ONION_AFTER);
        assert!(frames[1].2 < frames[0].2, "further is fainter");
    }

    #[test]
    fn the_onion_skin_picture_has_the_other_frames_tinted_and_not_this_one() {
        let mut document = timeline(3);
        for frame in 0..3 {
            document.go_to_frame(frame);
            let id = document.active_id();
            quill_engine::frames::AnimatedCommand::set(&mut document, id, true);
            document.active_layer_mut().surface.set_pixel(
                frame as i32 * 5,
                5,
                quill_engine::tile::Rgba8::new(0, 0, 0, 255),
            );
        }
        document.go_to_frame(1);
        let onion = Onion {
            on: true,
            before: 1,
            after: 1,
            strength: 0.4,
        };
        let skin = onion_skin(&document, &onion_frames(document.timeline(), 1, onion));
        let red = skin.pixel(0, 5);
        assert_eq!(&red[..3], &ONION_BEFORE);
        let blue = skin.pixel(10, 5);
        assert_eq!(&blue[..3], &ONION_AFTER);
        assert!(red[3] > 60 && red[3] < 140, "faint: {}", red[3]);
        assert_eq!(skin.pixel(5, 5)[3], 0, "the frame shown isn't in it");
    }

    #[test]
    fn a_still_background_is_not_in_the_onion_skin() {
        let mut document = quill_engine::frames::new_animation(20, 20, 3);
        let background = document.layers()[0].id();
        document
            .layer_mut(background)
            .unwrap()
            .surface
            .paint_pixels(
                quill_engine::surface::PixelRect::new(0, 0, 20, 20),
                |_, _| Some(quill_engine::tile::Rgba8::new(0, 200, 0, 255)),
            );
        document.go_to_frame(0);
        document.active_layer_mut().surface.set_pixel(
            3,
            3,
            quill_engine::tile::Rgba8::new(0, 0, 0, 255),
        );
        document.go_to_frame(1);
        let onion = Onion {
            on: true,
            before: 1,
            after: 1,
            strength: 0.4,
        };
        let skin = onion_skin(&document, &onion_frames(document.timeline(), 1, onion));
        assert_eq!(&skin.pixel(3, 3)[..3], &ONION_BEFORE, "frame 0's drawing");
        assert_eq!(skin.pixel(10, 10)[3], 0, "no background");
    }
}
