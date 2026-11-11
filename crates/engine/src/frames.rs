//! Frames: drawing an animation by hand.
//!
//! A picture has a timeline of one or more frames, and one of them is
//! shown. Each layer is either **still** (one drawing, the same on every
//! frame: a background, a guide) or **animated** (its own drawing on
//! every frame). Going to another frame swaps which drawing each animated
//! layer shows; nothing is copied, and everything that paints on a layer
//! (brush, fill, filters, selections, transform, scripts) paints on the
//! frame shown without knowing frames exist.
//!
//! Drawings share their tiles (see `surface.rs`), so a duplicated frame
//! costs almost no memory until it's drawn on, and a blank frame none.
//!
//! Every change to the frames (add, duplicate, delete, move, hold, a
//! layer switched between still and animated, fps, loop, the named
//! stretches) is an undoable command here. Going to another frame isn't
//! a change: `History` notes which frame each step was made on and goes
//! back there before undoing it.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::history::Command;
use crate::layer::LayerId;
use crate::surface::Surface;
use crate::tile::TILE_PIXELS;

/// The most frames a picture may have.
pub const MAX_FRAMES: usize = 1000;
/// The longest a frame may be held, in frames.
pub const MAX_HOLD: u32 = 100;

/// One frame of the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// How many frames' time it's shown for (1 = once, 2 = twice as long).
    pub hold: u32,
}

impl Default for FrameInfo {
    fn default() -> Self {
        Self { hold: 1 }
    }
}

/// A named run of frames: one animation of a sprite ("idle", "run").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stretch {
    pub name: String,
    /// The first frame, from 0.
    pub start: usize,
    /// The frame after the last one.
    pub end: usize,
    pub fps: f32,
    #[serde(rename = "loop")]
    pub looping: bool,
}

/// The frames of a picture and how they play.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub(crate) frames: Vec<FrameInfo>,
    /// The frame shown.
    pub(crate) current: usize,
    /// For the whole timeline, when there are no stretches.
    pub fps: f32,
    pub looping: bool,
    pub stretches: Vec<Stretch>,
    /// Where the character stands, in canvas pixels (the sprite sheet's
    /// pivot). `None`: the bottom middle of the canvas.
    pub pivot: Option<[f32; 2]>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            frames: vec![FrameInfo::default()],
            current: 0,
            fps: 12.0,
            looping: true,
            stretches: Vec::new(),
            pivot: None,
        }
    }
}

impl Timeline {
    pub fn frames(&self) -> &[FrameInfo] {
        &self.frames
    }

    /// The stretch frame `frame` is in, if any.
    pub fn stretch_of(&self, frame: usize) -> Option<&Stretch> {
        self.stretches
            .iter()
            .find(|s| (s.start..s.end).contains(&frame))
    }

    /// The frames that play together with `frame`, and how fast: its
    /// stretch, or the whole timeline.
    pub fn playing_range(&self, frame: usize) -> (usize, usize, f32, bool) {
        match self.stretch_of(frame) {
            Some(s) => (s.start, s.end, s.fps, s.looping),
            None => (0, self.frames.len(), self.fps, self.looping),
        }
    }

    /// The timing settings without the frames (what `TimelineCommand`
    /// changes).
    pub fn settings(&self) -> TimelineSettings {
        TimelineSettings {
            fps: self.fps,
            looping: self.looping,
            stretches: self.stretches.clone(),
            pivot: self.pivot,
        }
    }

    fn apply(&mut self, settings: TimelineSettings) {
        self.fps = settings.fps;
        self.looping = settings.looping;
        self.stretches = settings.stretches;
        self.pivot = settings.pivot;
    }

    /// Stretches after a frame is put in at `at`: one that the new frame
    /// lands inside (or right at the end of) grows, those after it move.
    fn stretches_after_insert(&mut self, at: usize) {
        for s in &mut self.stretches {
            if s.start < at && at <= s.end {
                s.end += 1;
            } else if s.start >= at {
                s.start += 1;
                s.end += 1;
            }
        }
    }

    /// Stretches after the frame at `at` is taken out; one left empty goes.
    fn stretches_after_remove(&mut self, at: usize) {
        for s in &mut self.stretches {
            if at < s.start {
                s.start -= 1;
                s.end -= 1;
            } else if at < s.end {
                s.end -= 1;
            }
        }
        self.stretches.retain(|s| s.end > s.start);
    }
}

/// fps, loop, the stretches and the pivot, together.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineSettings {
    pub fps: f32,
    pub looping: bool,
    pub stretches: Vec<Stretch>,
    pub pivot: Option<[f32; 2]>,
}

// ---- The document's frames -------------------------------------------------------

impl Document {
    /// How many frames the picture has (1 if it isn't animated).
    pub fn frame_count(&self) -> usize {
        self.timeline.frames.len()
    }

    /// The frame shown, from 0.
    pub fn frame(&self) -> usize {
        self.timeline.current
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// True if the picture has anything of an animation: more than one
    /// frame, an animated layer, a named stretch or a hold. Such a picture
    /// is saved as file version 2; any other exactly as before.
    pub fn is_animated(&self) -> bool {
        self.frame_count() > 1
            || self.layers().iter().any(|layer| layer.animated())
            || !self.timeline.stretches.is_empty()
            || self.timeline.frames.iter().any(|f| f.hold != 1)
            || self.timeline.pivot.is_some()
    }

    /// Shows frame `frame` (the last one if there aren't that many):
    /// every animated layer shows its drawing for it. Not a change to the
    /// picture, so not an undo step: use `History::go_to_frame` in the
    /// app, so undo knows where you are.
    pub fn go_to_frame(&mut self, frame: usize) {
        let frame = frame.min(self.frame_count() - 1);
        let from = self.timeline.current;
        if frame == from {
            return;
        }
        for layer in self.layers_mut() {
            let Some(drawings) = &mut layer.drawings else {
                continue;
            };
            // Everywhere either drawing has paint (or was just changed)
            // must be redrawn on screen and in the layer's thumbnail.
            let mut changed: HashSet<_> = layer.surface.tiles().map(|(c, _)| c).collect();
            changed.extend(layer.surface.take_dirty_tiles());
            changed.extend(layer.surface.take_thumbnail_tiles());
            let locked = layer.surface.locked;
            drawings[from] = std::mem::take(&mut layer.surface);
            layer.surface = std::mem::take(&mut drawings[frame]);
            // The alpha lock is the layer's, not a frame's.
            layer.surface.locked = locked;
            drawings[from].locked = false;
            changed.extend(layer.surface.tiles().map(|(c, _)| c));
            layer.surface.mark_changed(changed);
        }
        self.timeline.current = frame;
    }

    /// A copy of the picture showing frame `frame`: for exports, the
    /// onion skin and the timeline's little pictures. Cheap: the copy
    /// shares every tile.
    pub fn frame_picture(&self, frame: usize) -> Document {
        let mut copy = self.clone();
        copy.go_to_frame(frame);
        copy
    }

    /// How much memory the pixels of every layer, frame and drawing use
    /// together, in bytes: a tile shared by several frames is counted
    /// once.
    pub fn pixel_memory(&self) -> usize {
        let mut seen: HashSet<*const crate::tile::Tile> = HashSet::new();
        for layer in self.layers() {
            let all = std::iter::once(&layer.surface).chain(layer.drawings.iter().flatten());
            for surface in all {
                for tile in surface.shared_tiles() {
                    seen.insert(Arc::as_ptr(tile));
                }
            }
        }
        seen.len() * TILE_PIXELS * 4
    }

    /// Puts a frame in at `at`: empty on every animated layer, or (with
    /// `copy_of`) a copy of that frame's drawings. Returns false if there
    /// are as many frames as allowed.
    fn insert_frame(&mut self, at: usize, copy_of: Option<usize>, info: FrameInfo) -> bool {
        let count = self.frame_count();
        if count >= MAX_FRAMES {
            return false;
        }
        let at = at.min(count);
        let current = self.timeline.current;
        for layer in self.layers_mut() {
            let new = match copy_of {
                Some(source) => {
                    let mut copy = layer.drawing(source, current).clone();
                    copy.locked = false;
                    let _ = copy.take_dirty_tiles();
                    let _ = copy.take_thumbnail_tiles();
                    copy
                }
                None => Surface::new(),
            };
            if let Some(drawings) = &mut layer.drawings {
                drawings.insert(at, new);
            }
        }
        self.timeline.frames.insert(at, info);
        if at <= current {
            self.timeline.current += 1;
        }
        self.timeline.stretches_after_insert(at);
        true
    }

    /// Takes the frame at `at` out. Not the last frame: returns `None`.
    fn remove_frame(&mut self, at: usize) -> Option<RemovedFrame> {
        if self.frame_count() <= 1 || at >= self.frame_count() {
            return None;
        }
        if at == self.timeline.current {
            // Show a neighbour first: the frame shown can't go.
            let next = if at + 1 < self.frame_count() {
                at + 1
            } else {
                at - 1
            };
            self.go_to_frame(next);
        }
        let mut drawings = Vec::new();
        for layer in self.layers_mut() {
            let id = layer.id();
            if let Some(list) = &mut layer.drawings {
                drawings.push((id, list.remove(at)));
            }
        }
        let info = self.timeline.frames.remove(at);
        if at < self.timeline.current {
            self.timeline.current -= 1;
        }
        self.timeline.stretches_after_remove(at);
        Some(RemovedFrame { info, drawings })
    }

    /// Puts a removed frame back where it was.
    fn put_back_frame(&mut self, at: usize, removed: RemovedFrame) {
        let RemovedFrame { info, mut drawings } = removed;
        let current = self.timeline.current;
        for layer in self.layers_mut() {
            let id = layer.id();
            if let Some(list) = &mut layer.drawings {
                let drawing = drawings
                    .iter()
                    .position(|(owner, _)| *owner == id)
                    .map(|i| drawings.swap_remove(i).1)
                    .unwrap_or_default();
                list.insert(at, drawing);
            }
        }
        self.timeline.frames.insert(at, info);
        if at <= current {
            self.timeline.current += 1;
        }
    }

    /// Moves the frame at `from` to `to` (both in the timeline as it is).
    fn move_frame(&mut self, from: usize, to: usize) {
        let count = self.frame_count();
        if from >= count || to >= count || from == to {
            return;
        }
        for layer in self.layers_mut() {
            if let Some(list) = &mut layer.drawings {
                let drawing = list.remove(from);
                list.insert(to, drawing);
            }
        }
        let info = self.timeline.frames.remove(from);
        self.timeline.frames.insert(to, info);
        // The frame shown moves with its drawing (the live one is the
        // stand-in's place in the list).
        let current = self.timeline.current;
        self.timeline.current = if current == from {
            to
        } else if from < current && current <= to {
            current - 1
        } else if to <= current && current < from {
            current + 1
        } else {
            current
        };
    }

    fn layers_mut(&mut self) -> impl Iterator<Item = &mut crate::layer::Layer> {
        self.layers_mut_slice().iter_mut()
    }
}

/// A frame taken out of the picture, kept for undo.
struct RemovedFrame {
    info: FrameInfo,
    /// Each animated layer's drawing on it.
    drawings: Vec<(LayerId, Surface)>,
}

fn drawing_bytes(surface: &Surface) -> usize {
    surface
        .shared_tiles()
        .filter(|tile| Arc::strong_count(tile) == 1)
        .count()
        * TILE_PIXELS
        * 4
}

// ---- New animations -------------------------------------------------------------

/// The animations a sprite template can start with: name, frames, fps.
pub const SPRITE_ANIMATIONS: &[(&str, usize, f32)] = &[
    ("idle", 4, 6.0),
    ("run", 8, 12.0),
    ("jump", 6, 10.0),
    ("attack", 5, 12.0),
];

/// A new animation to draw: a still "Background" layer and an animated
/// "Drawing" layer (selected), with `frames` empty frames.
pub fn new_animation(width: u32, height: u32, frames: usize) -> Document {
    let mut document = Document::new(width, height);
    let background = document.active_id();
    if let Some(layer) = document.layer_mut(background) {
        let mut props = layer.props();
        props.name = "Background".into();
        layer.set_props(props);
    }
    let _ = crate::layer_commands::LayerPresenceCommand::insert_layer(&mut document, "Drawing", 1);
    let drawing = document.active_id();
    let _ = AnimatedCommand::set(&mut document, drawing, true);
    for _ in 1..frames.clamp(1, MAX_FRAMES) {
        let end = document.frame_count();
        let _ = FrameCommand::add(&mut document, end);
    }
    document.go_to_frame(0);
    document
}

/// A new pixel sprite `size` x `size`, with one stretch per animation
/// chosen (from `SPRITE_ANIMATIONS`), the pivot at the bottom middle and
/// a see-through background (no paper shows in exports anyway).
pub fn new_sprite(size: u32, animations: &[(&str, usize, f32)]) -> Document {
    let total: usize = animations.iter().map(|a| a.1).sum::<usize>().max(1);
    let mut document = new_animation(size, size, total);
    let mut start = 0;
    let mut settings = document.timeline.settings();
    for &(name, frames, fps) in animations {
        settings.stretches.push(Stretch {
            name: name.to_owned(),
            start,
            end: start + frames,
            fps,
            looping: name != "jump" && name != "attack",
        });
        start += frames;
    }
    settings.pivot = Some([size as f32 / 2.0, size as f32]);
    if let Some(first) = animations.first() {
        settings.fps = first.2;
    }
    let _ = TimelineCommand::apply(&mut document, settings);
    document
}

// ---- Exporting ------------------------------------------------------------------

/// One animation of a picture with frames: a stretch, or (with no
/// stretches) the whole timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    pub name: String,
    /// Its frames, in order.
    pub frames: Vec<usize>,
    /// Each frame's hold, in the same order.
    pub holds: Vec<u32>,
    pub fps: f32,
    pub looping: bool,
}

impl Played {
    /// Its frames with each repeated for its hold: what a GIF or a video
    /// shows, one per tick of `fps`.
    pub fn ticks(&self) -> Vec<usize> {
        self.frames
            .iter()
            .zip(&self.holds)
            .flat_map(|(&frame, &hold)| std::iter::repeat_n(frame, hold.max(1) as usize))
            .collect()
    }
}

impl Document {
    /// The picture's animations: one per named stretch, in order, or the
    /// whole timeline called `name` when there are none. Frames outside
    /// every stretch belong to none.
    pub fn animations(&self, name: &str) -> Vec<Played> {
        let timeline = &self.timeline;
        let holds = |range: std::ops::Range<usize>| -> Vec<u32> {
            timeline.frames[range].iter().map(|f| f.hold).collect()
        };
        if timeline.stretches.is_empty() {
            let count = self.frame_count();
            return vec![Played {
                name: name.to_owned(),
                frames: (0..count).collect(),
                holds: holds(0..count),
                fps: timeline.fps,
                looping: timeline.looping,
            }];
        }
        timeline
            .stretches
            .iter()
            .map(|s| Played {
                name: s.name.clone(),
                frames: (s.start..s.end).collect(),
                holds: holds(s.start..s.end),
                fps: s.fps,
                looping: s.looping,
            })
            .collect()
    }

    /// Where the character stands: the pivot, or the bottom middle.
    pub fn pivot(&self) -> [f32; 2] {
        self.timeline
            .pivot
            .unwrap_or([self.width() as f32 / 2.0, self.height() as f32])
    }

    /// Every frame painted, see-through, by number (only those `wanted`).
    pub fn frame_images(
        &self,
        wanted: &[usize],
    ) -> std::collections::BTreeMap<usize, crate::image::Image> {
        wanted
            .iter()
            .map(|&frame| {
                let picture = self.frame_picture(frame);
                let image = crate::image::Image::of_document(
                    &picture,
                    crate::document::Background::Transparent,
                );
                (frame, image)
            })
            .collect()
    }

    /// The sprite sheet of this picture's animations (`scale` times
    /// bigger, sharp pixels) and its JSON: the same format as `quill
    /// export` of a script, a held frame listed once per tick.
    pub fn sprite_sheet(
        &self,
        name: &str,
        image_name: &str,
        scale: u32,
    ) -> Result<(crate::image::Image, String), String> {
        let animations = self.animations(name);
        let all: Vec<usize> = animations.iter().flat_map(|a| a.frames.clone()).collect();
        let images = self.frame_images(&all);
        let sheet: Vec<crate::sprites::SheetAnimation> = animations
            .iter()
            .map(|a| crate::sprites::SheetAnimation {
                name: a.name.clone(),
                frames: a.frames.iter().map(|f| images[f].clone()).collect(),
                fps: a.fps,
                looping: a.looping,
                holds: a.holds.clone(),
            })
            .collect();
        crate::sprites::sprite_sheet(&sheet, image_name, self.pivot(), scale)
    }
}

// ---- Undoable frame changes ------------------------------------------------------

/// Adding, duplicating, deleting or moving a frame, or changing how long
/// one is held.
pub struct FrameCommand {
    name: &'static str,
    change: FrameChange,
    /// The stretches before and after (adding or deleting a frame moves
    /// their ends).
    stretches: [Vec<Stretch>; 2],
    /// The frame shown before and after: undo and redo show it again.
    shown: [usize; 2],
}

enum FrameChange {
    /// A frame was put in at `at`; while undone, it's kept here.
    Added {
        at: usize,
        stored: Option<RemovedFrame>,
    },
    /// The frame at `at` was taken out and is kept here; while undone,
    /// it's back in the picture.
    Deleted {
        at: usize,
        stored: Option<RemovedFrame>,
    },
    Moved {
        from: usize,
        to: usize,
    },
    Held {
        at: usize,
        holds: [u32; 2],
    },
}

impl FrameCommand {
    /// Puts an empty frame in at `at` (`frame_count()` for the end).
    pub fn add(document: &mut Document, at: usize) -> Option<Self> {
        let before = document.timeline.stretches.clone();
        let shown = document.frame();
        let at = at.min(document.frame_count());
        document
            .insert_frame(at, None, FrameInfo::default())
            .then(|| Self {
                name: "Add frame",
                change: FrameChange::Added { at, stored: None },
                stretches: [before, document.timeline.stretches.clone()],
                shown: [shown, document.frame()],
            })
    }

    /// Puts a copy of frame `frame` right after it.
    pub fn duplicate(document: &mut Document, frame: usize) -> Option<Self> {
        if frame >= document.frame_count() {
            return None;
        }
        let before = document.timeline.stretches.clone();
        let shown = document.frame();
        let info = document.timeline.frames[frame];
        document
            .insert_frame(frame + 1, Some(frame), info)
            .then(|| Self {
                name: "Duplicate frame",
                change: FrameChange::Added {
                    at: frame + 1,
                    stored: None,
                },
                stretches: [before, document.timeline.stretches.clone()],
                shown: [shown, document.frame()],
            })
    }

    /// Deletes frame `frame` (not the only one).
    pub fn delete(document: &mut Document, frame: usize) -> Option<Self> {
        let before = document.timeline.stretches.clone();
        let shown = document.frame();
        let stored = document.remove_frame(frame)?;
        Some(Self {
            name: "Delete frame",
            change: FrameChange::Deleted {
                at: frame,
                stored: Some(stored),
            },
            stretches: [before, document.timeline.stretches.clone()],
            shown: [shown, document.frame()],
        })
    }

    /// Moves frame `from` to place `to`.
    pub fn move_to(document: &mut Document, from: usize, to: usize) -> Option<Self> {
        let count = document.frame_count();
        if from >= count || to >= count || from == to {
            return None;
        }
        let stretches = document.timeline.stretches.clone();
        let shown = document.frame();
        document.move_frame(from, to);
        Some(Self {
            name: "Move frame",
            change: FrameChange::Moved { from, to },
            stretches: [stretches.clone(), stretches],
            shown: [shown, document.frame()],
        })
    }

    /// Shows frame `frame` for `hold` frames' time (1 to `MAX_HOLD`).
    pub fn hold(document: &mut Document, frame: usize, hold: u32) -> Option<Self> {
        let hold = hold.clamp(1, MAX_HOLD);
        let info = document.timeline.frames.get_mut(frame)?;
        if info.hold == hold {
            return None;
        }
        let before = std::mem::replace(&mut info.hold, hold);
        let stretches = document.timeline.stretches.clone();
        let shown = document.frame();
        Some(Self {
            name: "Hold frame",
            change: FrameChange::Held {
                at: frame,
                holds: [before, hold],
            },
            stretches: [stretches.clone(), stretches],
            shown: [shown, shown],
        })
    }
}

impl Command for FrameCommand {
    fn name(&self) -> &str {
        self.name
    }

    fn undo(&mut self, document: &mut Document) {
        match &mut self.change {
            FrameChange::Added { at, stored } => *stored = document.remove_frame(*at),
            FrameChange::Deleted { at, stored } => {
                if let Some(frame) = stored.take() {
                    document.put_back_frame(*at, frame);
                }
            }
            FrameChange::Moved { from, to } => document.move_frame(*to, *from),
            FrameChange::Held { at, holds } => {
                if let Some(info) = document.timeline.frames.get_mut(*at) {
                    info.hold = holds[0];
                }
            }
        }
        document.timeline.stretches = self.stretches[0].clone();
        document.go_to_frame(self.shown[0]);
    }

    fn redo(&mut self, document: &mut Document) {
        match &mut self.change {
            FrameChange::Added { at, stored } => {
                if let Some(frame) = stored.take() {
                    document.put_back_frame(*at, frame);
                }
            }
            FrameChange::Deleted { at, stored, .. } => *stored = document.remove_frame(*at),
            FrameChange::Moved { from, to } => document.move_frame(*from, *to),
            FrameChange::Held { at, holds } => {
                if let Some(info) = document.timeline.frames.get_mut(*at) {
                    info.hold = holds[1];
                }
            }
        }
        document.timeline.stretches = self.stretches[1].clone();
        document.go_to_frame(self.shown[1]);
    }

    fn memory_size(&self) -> usize {
        let stored = match &self.change {
            FrameChange::Added { stored, .. } | FrameChange::Deleted { stored, .. } => {
                stored.as_ref()
            }
            _ => None,
        };
        stored.map_or(0, |frame| {
            frame.drawings.iter().map(|(_, d)| drawing_bytes(d)).sum()
        })
    }

    fn frame_after(&self) -> Option<usize> {
        Some(self.shown[1])
    }
}

/// Switching a layer between still (one drawing on every frame) and
/// animated (a drawing per frame).
pub struct AnimatedCommand {
    layer: LayerId,
    /// The other frames' drawings while the layer is still (kept for
    /// undo after switching it to still).
    stored: Option<Vec<Surface>>,
    animated: bool,
}

impl AnimatedCommand {
    /// Makes layer `layer` animated (its drawing copied onto every frame,
    /// sharing its tiles) or still (it keeps the frame shown's drawing;
    /// the others are kept here for undo). `None` if it already is.
    pub fn set(document: &mut Document, layer: LayerId, animated: bool) -> Option<Self> {
        let frames = document.frame_count();
        let current = document.frame();
        let target = document.layer_mut(layer)?;
        if target.animated() == animated {
            return None;
        }
        let stored = if animated {
            let mut copy = target.surface.clone();
            let _ = copy.take_dirty_tiles();
            let _ = copy.take_thumbnail_tiles();
            copy.locked = false;
            let mut drawings = vec![copy; frames];
            drawings[current] = Surface::new();
            target.drawings = Some(drawings);
            None
        } else {
            target.drawings.take()
        };
        Some(Self {
            layer,
            stored,
            animated,
        })
    }

    fn swap(&mut self, document: &mut Document) {
        if let Some(layer) = document.layer_mut(self.layer) {
            std::mem::swap(&mut layer.drawings, &mut self.stored);
            layer.surface.mark_all_dirty();
        }
    }
}

impl Command for AnimatedCommand {
    fn name(&self) -> &str {
        if self.animated {
            "Animate layer"
        } else {
            "Make layer still"
        }
    }

    fn undo(&mut self, document: &mut Document) {
        self.swap(document);
    }

    fn redo(&mut self, document: &mut Document) {
        self.swap(document);
    }

    fn memory_size(&self) -> usize {
        self.stored.iter().flatten().map(drawing_bytes).sum()
    }
}

/// Changing fps, loop, the named stretches or the pivot.
pub struct TimelineCommand {
    settings: [TimelineSettings; 2],
}

impl TimelineCommand {
    /// Changes the timing settings to `after`. `None` if nothing changes.
    pub fn apply(document: &mut Document, mut after: TimelineSettings) -> Option<Self> {
        let count = document.frame_count();
        after.fps = after.fps.clamp(1.0, 60.0);
        for s in &mut after.stretches {
            s.fps = s.fps.clamp(1.0, 60.0);
            s.end = s.end.min(count);
        }
        after
            .stretches
            .retain(|s| s.end > s.start && !s.name.is_empty());
        let before = document.timeline.settings();
        if before == after {
            return None;
        }
        document.timeline.apply(after.clone());
        Some(Self {
            settings: [before, after],
        })
    }
}

impl Command for TimelineCommand {
    fn name(&self) -> &str {
        "Change timing"
    }

    fn undo(&mut self, document: &mut Document) {
        document.timeline.apply(self.settings[0].clone());
    }

    fn redo(&mut self, document: &mut Document) {
        document.timeline.apply(self.settings[1].clone());
    }

    fn memory_size(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Background;
    use crate::history::{History, TileCommand};
    use crate::image::Image;
    use crate::surface::{PixelRect, TileCoord};
    use crate::tile::Rgba8;
    use std::collections::HashMap;

    const RED: Rgba8 = Rgba8::new(255, 0, 0, 255);
    const BLUE: Rgba8 = Rgba8::new(0, 0, 255, 255);

    /// A 600 x 300 picture: a still "Background" (blue at the top left)
    /// and an animated "Duck" layer, active, with `frames` frames.
    fn animation(frames: usize) -> Document {
        let mut document = Document::new(600, 300);
        let background = document.active_id();
        document
            .layer_mut(background)
            .unwrap()
            .surface
            .set_pixel(1, 1, BLUE);
        let _ = crate::layer_commands::LayerPresenceCommand::add_layer(&mut document);
        let duck = document.active_id();
        AnimatedCommand::set(&mut document, duck, true).unwrap();
        for _ in 1..frames {
            let end = document.frame_count();
            FrameCommand::add(&mut document, end).unwrap();
        }
        document
    }

    /// Paints a red dot at `x` on the active layer, as an undo step.
    fn dot(document: &mut Document, history: &mut History, x: i32) {
        let layer = document.active_id();
        let surface = &mut document.active_layer_mut().surface;
        let coord = TileCoord::containing(x, 50);
        let before = HashMap::from([(coord, surface.tile_shared(coord))]);
        surface.set_pixel(x, 50, RED);
        history.push(Box::new(TileCommand::new("Dot", layer, before)));
    }

    fn red_at(document: &Document, x: i32) -> bool {
        document.active_layer().surface.pixel(x, 50) == RED
    }

    #[test]
    fn each_frame_has_its_own_drawing_and_a_still_layer_is_on_all_of_them() {
        let mut document = animation(3);
        let mut history = History::default();
        dot(&mut document, &mut history, 10);
        history.go_to_frame(&mut document, 1);
        assert!(!red_at(&document, 10), "frame 1 is empty");
        dot(&mut document, &mut history, 20);
        history.go_to_frame(&mut document, 0);
        assert!(red_at(&document, 10) && !red_at(&document, 20));
        // The background is the same on every frame.
        for frame in 0..3 {
            let picture = document.frame_picture(frame);
            assert_eq!(picture.layers()[0].surface.pixel(1, 1), BLUE);
        }
        assert_eq!(
            document
                .frame_picture(1)
                .active_layer()
                .surface
                .pixel(20, 50),
            RED
        );
        // frame_picture doesn't move the frame shown.
        assert_eq!(document.frame(), 0);
    }

    #[test]
    fn going_to_a_frame_redraws_where_either_drawing_has_paint() {
        let mut document = animation(2);
        let mut history = History::default();
        dot(&mut document, &mut history, 10); // tile (0, 0) on frame 0
        history.go_to_frame(&mut document, 1);
        dot(&mut document, &mut history, 300); // tile (1, 0) on frame 1
        let _ = document.take_dirty_tiles();
        let _ = document.active_layer_mut().surface.take_thumbnail_tiles();
        history.go_to_frame(&mut document, 0);
        let mut dirty = document.take_dirty_tiles();
        dirty.sort();
        assert_eq!(dirty, [TileCoord::new(0, 0), TileCoord::new(1, 0)]);
        let mut thumbs = document.active_layer_mut().surface.take_thumbnail_tiles();
        thumbs.sort();
        assert_eq!(thumbs, [TileCoord::new(0, 0), TileCoord::new(1, 0)]);
    }

    #[test]
    fn undo_goes_back_to_the_frame_the_change_was_made_on() {
        let mut document = animation(4);
        let mut history = History::default();
        history.go_to_frame(&mut document, 3);
        dot(&mut document, &mut history, 10);
        history.go_to_frame(&mut document, 0);
        dot(&mut document, &mut history, 30);
        history.go_to_frame(&mut document, 2);
        // Undo the dot on frame 0: we're taken there, and it's gone.
        assert!(history.undo(&mut document));
        assert_eq!(document.frame(), 0);
        assert!(!red_at(&document, 30));
        // Then the one on frame 3, from wherever we are.
        history.go_to_frame(&mut document, 1);
        assert!(history.undo(&mut document));
        assert_eq!(document.frame(), 3);
        assert!(!red_at(&document, 10));
        // Redo puts each back on its own frame.
        assert!(history.redo(&mut document));
        assert_eq!(document.frame(), 3);
        assert!(red_at(&document, 10));
        assert!(history.redo(&mut document));
        assert_eq!(document.frame(), 0);
        assert!(red_at(&document, 30));
        assert!(
            !document
                .frame_picture(1)
                .active_layer()
                .surface
                .pixel(10, 50)
                .a
                > 0
        );
    }

    #[test]
    fn adding_duplicating_moving_and_deleting_frames_undo() {
        let mut document = animation(1);
        let mut history = History::default();
        dot(&mut document, &mut history, 10);
        // Duplicate frame 0: frame 1 is a copy, and drawing on it leaves 0 alone.
        let step = FrameCommand::duplicate(&mut document, 0).unwrap();
        history.push(Box::new(step));
        assert_eq!(document.frame_count(), 2);
        history.go_to_frame(&mut document, 1);
        assert!(red_at(&document, 10), "a copy");
        dot(&mut document, &mut history, 40);
        assert!(
            !document
                .frame_picture(0)
                .active_layer()
                .surface
                .pixel(40, 50)
                .a
                > 0
        );
        // Move frame 1 to the front: the frame shown moves with it.
        history.push(Box::new(
            FrameCommand::move_to(&mut document, 1, 0).unwrap(),
        ));
        assert_eq!(document.frame(), 0);
        assert!(red_at(&document, 40));
        assert!(
            !document
                .frame_picture(1)
                .active_layer()
                .surface
                .pixel(40, 50)
                .a
                > 0
        );
        // Hold it, then delete the other one.
        history.push(Box::new(FrameCommand::hold(&mut document, 0, 3).unwrap()));
        assert_eq!(document.timeline().frames()[0].hold, 3);
        history.push(Box::new(FrameCommand::delete(&mut document, 1).unwrap()));
        assert_eq!(document.frame_count(), 1);
        assert!(
            FrameCommand::delete(&mut document, 0).is_none(),
            "not the last frame"
        );
        // Undo it all, step by step.
        assert!(history.undo(&mut document)); // the delete
        assert_eq!(document.frame_count(), 2);
        assert!(
            !document
                .frame_picture(1)
                .active_layer()
                .surface
                .pixel(40, 50)
                .a
                > 0
        );
        assert!(
            document
                .frame_picture(1)
                .active_layer()
                .surface
                .pixel(10, 50)
                == RED
        );
        assert!(history.undo(&mut document)); // the hold
        assert_eq!(document.timeline().frames()[0].hold, 1);
        assert!(history.undo(&mut document)); // the move
        assert!(red_at(&document, 40) && document.frame() == 1);
        assert!(history.undo(&mut document)); // the dot at 40
        assert!(!red_at(&document, 40));
        assert!(history.undo(&mut document)); // the duplicate
        assert_eq!((document.frame_count(), document.frame()), (1, 0));
        assert!(red_at(&document, 10));
        // And redo all of it.
        while history.redo(&mut document) {}
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.timeline().frames()[0].hold, 3);
        assert!(red_at(&document, 40) && red_at(&document, 10));
    }

    #[test]
    fn deleting_the_frame_shown_shows_a_neighbour_and_undo_brings_it_back() {
        let mut document = animation(3);
        let mut history = History::default();
        history.go_to_frame(&mut document, 2);
        dot(&mut document, &mut history, 10);
        history.push(Box::new(FrameCommand::delete(&mut document, 2).unwrap()));
        assert_eq!((document.frame_count(), document.frame()), (2, 1));
        assert!(!red_at(&document, 10));
        // A change now is made on frame 1 (history knows without being
        // told), and undoing it from frame 0 goes back there.
        dot(&mut document, &mut history, 90);
        history.go_to_frame(&mut document, 0);
        history.undo(&mut document);
        assert_eq!(document.frame(), 1);
        assert!(!red_at(&document, 90));
        history.undo(&mut document);
        assert_eq!((document.frame_count(), document.frame()), (3, 2));
        assert!(red_at(&document, 10));
    }

    #[test]
    fn stretches_grow_and_shrink_with_their_frames() {
        let mut document = animation(8);
        let settings = TimelineSettings {
            stretches: vec![
                Stretch {
                    name: "idle".into(),
                    start: 0,
                    end: 4,
                    fps: 6.0,
                    looping: true,
                },
                Stretch {
                    name: "run".into(),
                    start: 4,
                    end: 8,
                    fps: 12.0,
                    looping: true,
                },
            ],
            ..document.timeline().settings()
        };
        let mut history = History::default();
        history.push(Box::new(
            TimelineCommand::apply(&mut document, settings).unwrap(),
        ));
        // A frame after idle's last one belongs to idle; run moves along.
        history.push(Box::new(FrameCommand::duplicate(&mut document, 3).unwrap()));
        let spans: Vec<(usize, usize)> = document
            .timeline()
            .stretches
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(spans, [(0, 5), (5, 9)]);
        assert_eq!(document.timeline().playing_range(6), (5, 9, 12.0, true));
        history.push(Box::new(FrameCommand::delete(&mut document, 0).unwrap()));
        let spans: Vec<(usize, usize)> = document
            .timeline()
            .stretches
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(spans, [(0, 4), (4, 8)]);
        history.undo(&mut document);
        history.undo(&mut document);
        let spans: Vec<(usize, usize)> = document
            .timeline()
            .stretches
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(spans, [(0, 4), (4, 8)]);
        history.undo(&mut document);
        assert!(document.timeline().stretches.is_empty());
    }

    #[test]
    fn a_layer_switched_between_still_and_animated_keeps_its_drawings_through_undo() {
        let mut document = animation(3);
        let mut history = History::default();
        let background = document.layers()[0].id();
        // Animate the background: every frame starts with its drawing.
        history.push(Box::new(
            AnimatedCommand::set(&mut document, background, true).unwrap(),
        ));
        assert!(document.layers()[0].animated());
        history.go_to_frame(&mut document, 2);
        document.set_active(background);
        dot(&mut document, &mut history, 70);
        assert_eq!(
            document.frame_picture(0).layers()[0].surface.pixel(1, 1),
            BLUE
        );
        assert_eq!(
            document.frame_picture(0).layers()[0]
                .surface
                .pixel(70, 50)
                .a,
            0
        );
        // Make it still on frame 2: it keeps frame 2's drawing everywhere.
        history.push(Box::new(
            AnimatedCommand::set(&mut document, background, false).unwrap(),
        ));
        assert_eq!(
            document.frame_picture(0).layers()[0].surface.pixel(70, 50),
            RED
        );
        // Undo: animated again, and frame 0 is as it was.
        history.undo(&mut document);
        assert_eq!(
            document.frame_picture(0).layers()[0]
                .surface
                .pixel(70, 50)
                .a,
            0
        );
        assert_eq!(
            document.frame_picture(2).layers()[0].surface.pixel(70, 50),
            RED
        );
    }

    #[test]
    fn a_new_layer_in_an_animation_is_animated_and_the_alpha_lock_stays_with_the_layer() {
        let mut document = animation(3);
        let _ = crate::layer_commands::LayerPresenceCommand::add_layer(&mut document);
        assert!(document.active_layer().animated());
        let id = document.active_id();
        let mut props = document.active_layer().props();
        props.lock = true;
        document.layer_mut(id).unwrap().set_props(props);
        document.go_to_frame(2);
        assert!(document.active_layer().lock(), "locked on every frame");
        document.go_to_frame(0);
        assert!(document.active_layer().lock());
        // One frame: a new layer is still.
        let mut plain = Document::new(10, 10);
        assert!(!plain.new_layer().animated());
    }

    #[test]
    fn an_animated_picture_saves_and_opens_with_every_frame() {
        let mut document = animation(5);
        let mut history = History::default();
        for frame in 0..5 {
            history.go_to_frame(&mut document, frame);
            dot(&mut document, &mut history, 10 + 40 * frame as i32);
        }
        history.push(Box::new(FrameCommand::hold(&mut document, 1, 2).unwrap()));
        let settings = TimelineSettings {
            fps: 8.0,
            looping: false,
            stretches: vec![Stretch {
                name: "walk".into(),
                start: 0,
                end: 5,
                fps: 8.0,
                looping: true,
            }],
            pivot: Some([300.0, 290.0]),
        };
        history.push(Box::new(
            TimelineCommand::apply(&mut document, settings.clone()).unwrap(),
        ));
        history.go_to_frame(&mut document, 3);
        let mut bytes = std::io::Cursor::new(Vec::new());
        crate::file::write_document(&document, &mut bytes).unwrap();
        bytes.set_position(0);
        let opened = crate::file::read_document(bytes).unwrap();
        assert_eq!(opened.frame_count(), 5);
        assert_eq!(opened.frame(), 3);
        assert_eq!(opened.timeline().settings(), settings);
        assert_eq!(opened.timeline().frames()[1].hold, 2);
        assert!(opened.layers()[1].animated() && !opened.layers()[0].animated());
        for frame in 0..5 {
            let a = Image::of_document(&document.frame_picture(frame), Background::Paper);
            let b = Image::of_document(&opened.frame_picture(frame), Background::Paper);
            let same =
                (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)));
            assert!(same, "frame {frame} is the same");
        }
        // Each frame's dot is on its own frame only.
        assert_eq!(
            opened.frame_picture(4).layers()[1].surface.pixel(170, 50),
            RED
        );
        assert_eq!(
            opened.frame_picture(0).layers()[1].surface.pixel(170, 50).a,
            0
        );
    }

    #[test]
    fn a_picture_that_isnt_animated_is_saved_as_before_and_an_animation_as_version_2() {
        let description = |document: &Document| -> serde_json::Value {
            let mut bytes = std::io::Cursor::new(Vec::new());
            crate::file::write_document(document, &mut bytes).unwrap();
            bytes.set_position(0);
            let mut archive = zip::ZipArchive::new(bytes).unwrap();
            let names: Vec<String> = archive
                .file_names()
                .map(|name| name.unwrap().into_owned())
                .collect();
            let mut text = String::new();
            std::io::Read::read_to_string(
                &mut archive.by_name("document.json").unwrap(),
                &mut text,
            )
            .unwrap();
            let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
            value["files"] = serde_json::json!(names);
            value
        };
        let mut plain = Document::new(300, 200);
        plain.active_layer_mut().surface.set_pixel(5, 5, RED);
        let value = description(&plain);
        assert_eq!(value["version"], 1);
        assert!(value.get("animation").is_none());
        assert!(value["layers"][0].get("frames").is_none());
        assert!(
            value["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f == "layers/0/0_0.png")
        );
        let value = description(&animation(2));
        assert_eq!(value["version"], 2);
        assert_eq!(value["animation"]["holds"], serde_json::json!([1, 1]));
        assert_eq!(value["layers"][1]["frames"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn shared_tiles_are_saved_once() {
        // Eight duplicated frames of one drawing: one tile in the file.
        let mut document = animation(1);
        document.active_layer_mut().surface.set_pixel(5, 5, RED);
        for _ in 0..7 {
            FrameCommand::duplicate(&mut document, 0).unwrap();
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        crate::file::write_document(&document, &mut bytes).unwrap();
        bytes.set_position(0);
        let archive = zip::ZipArchive::new(bytes).unwrap();
        let tiles = archive
            .file_names()
            .filter(|n| n.as_ref().unwrap().starts_with("frames/"))
            .count();
        assert_eq!(tiles, 1);
    }

    #[test]
    fn a_sheet_has_a_row_per_stretch_and_lists_a_held_frame_once_per_tick() {
        let mut document = animation(5);
        for frame in 0..5 {
            document.go_to_frame(frame);
            document
                .active_layer_mut()
                .surface
                .set_pixel(frame as i32, 0, RED);
        }
        FrameCommand::hold(&mut document, 1, 2).unwrap();
        let settings = TimelineSettings {
            stretches: vec![
                Stretch {
                    name: "idle".into(),
                    start: 0,
                    end: 2,
                    fps: 6.0,
                    looping: true,
                },
                Stretch {
                    name: "run".into(),
                    start: 2,
                    end: 5,
                    fps: 12.0,
                    looping: false,
                },
            ],
            pivot: Some([300.0, 280.0]),
            ..document.timeline().settings()
        };
        TimelineCommand::apply(&mut document, settings).unwrap();
        let played = document.animations("walk");
        assert_eq!(played[0].ticks(), [0, 1, 1]);
        let (sheet, json) = document.sprite_sheet("walk", "duck.png", 1).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (3 * 600, 2 * 300));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["animations"]["idle"]["frames"],
            serde_json::json!([0, 1, 1])
        );
        assert_eq!(
            value["animations"]["run"]["frames"],
            serde_json::json!([3, 4, 5])
        );
        assert_eq!(value["animations"]["run"]["loop"], false);
        assert_eq!(value["pivot"], serde_json::json!([300.0, 280.0]));
        // Cell 4 (row 1, column 1) is frame 3: its dot at x 3.
        assert_eq!(sheet.pixel(600 + 3, 300)[3], 255);
        assert_eq!(sheet.pixel(600 + 2, 300)[3], 0);
        // No stretches: one animation, the whole timeline, by the name given.
        let plain = animation(2);
        assert_eq!(plain.animations("walk")[0].name, "walk");
        assert_eq!(plain.animations("walk")[0].frames, [0, 1]);
    }

    #[test]
    fn a_new_sprite_has_a_stretch_per_animation_and_its_pivot_on_the_ground() {
        let sprite = new_sprite(32, &SPRITE_ANIMATIONS[..2]);
        assert_eq!((sprite.width(), sprite.frame_count()), (32, 12));
        let names: Vec<(&str, usize, usize)> = sprite
            .timeline()
            .stretches
            .iter()
            .map(|s| (s.name.as_str(), s.start, s.end))
            .collect();
        assert_eq!(names, [("idle", 0, 4), ("run", 4, 12)]);
        assert_eq!(sprite.pivot(), [16.0, 32.0]);
        let layers: Vec<(&str, bool)> = sprite
            .layers()
            .iter()
            .map(|l| (l.name(), l.animated()))
            .collect();
        assert_eq!(layers, [("Background", false), ("Drawing", true)]);
        assert_eq!(sprite.active_layer().name(), "Drawing");
        assert_eq!(sprite.frame(), 0);
    }

    /// A stroke on frame `frame` of the Duck layer, left to right at y 50,
    /// kept inside `clip` and mirrored by `symmetry` if given.
    fn stroke_on(
        document: &mut Document,
        history: &mut History,
        frame: usize,
        clip: Option<std::sync::Arc<crate::fill::Mask>>,
        symmetry: Option<crate::stroke::Symmetry>,
    ) {
        history.go_to_frame(document, frame);
        let layer = document.active_id();
        let brush = crate::brush::BrushSettings {
            size: 8.0,
            ..Default::default()
        };
        let mut stroke = crate::stroke::Stroke::new(brush, document.bounds());
        if let Some(clip) = clip {
            stroke = stroke.clipped(clip);
        }
        if let Some(symmetry) = symmetry {
            stroke = stroke.mirrored(symmetry);
        }
        let surface = &mut document.active_layer_mut().surface;
        for x in (10..=200).step_by(5) {
            stroke.add_point(
                surface,
                crate::stroke::StrokePoint::new(x as f32, 50.0, 1.0),
            );
        }
        let before = stroke.finish(surface);
        history.push(Box::new(TileCommand::new("Stroke", layer, before)));
    }

    #[test]
    fn selections_symmetry_transform_and_references_work_on_every_frame() {
        let mut document = animation(3);
        let mut history = History::default();
        // A selection belongs to the picture: it stays put from frame to
        // frame, and paint on frame 2 stays inside it there.
        let limits = document.bounds();
        let selection = crate::selection::Selection::new(crate::selection::ellipse(
            100.0, 50.0, 30.0, 30.0, limits,
        ))
        .unwrap();
        document.set_selection(Some(selection));
        history.go_to_frame(&mut document, 2);
        assert!(document.selection().is_some(), "still selected on frame 2");
        let clip = document.clip();
        stroke_on(&mut document, &mut history, 2, clip, None);
        let on_2 = document.frame_picture(2).layers()[1]
            .surface
            .painted_area()
            .unwrap();
        assert!(
            on_2.left >= 69 && on_2.right <= 131,
            "inside the selection: {on_2:?}"
        );
        assert!(
            document.frame_picture(0).layers()[1]
                .surface
                .painted_area()
                .is_none()
        );
        document.set_selection(None);
        // Symmetry on frame 1: both halves there, nothing on frame 0.
        let symmetry = crate::stroke::Symmetry {
            centre: (300.0, 150.0),
            vertical: true,
            horizontal: false,
            angle: 0.0,
        };
        stroke_on(&mut document, &mut history, 1, None, Some(symmetry));
        let mirrored = document.frame_picture(1).layers()[1]
            .surface
            .painted_area()
            .unwrap();
        assert!(mirrored.left <= 10 && mirrored.right >= 590, "{mirrored:?}");
        assert!(
            document.frame_picture(0).layers()[1]
                .surface
                .painted_area()
                .is_none()
        );
        // Moving the drawing of frame 1 moves only frame 1's.
        history.go_to_frame(&mut document, 1);
        let duck = document.active_id();
        let before = crate::layer_transform::transform_layer(
            &mut document,
            duck,
            crate::transform::Transform::translate(0.0, 100.0),
        );
        history.push(Box::new(TileCommand::new("Move", duck, before)));
        let moved = document.frame_picture(1).layers()[1]
            .surface
            .painted_area()
            .unwrap();
        assert!(moved.top >= 140, "{moved:?}");
        let on_2_again = document.frame_picture(2).layers()[1]
            .surface
            .painted_area()
            .unwrap();
        assert_eq!(on_2_again, on_2, "frame 2 didn't move");
        // Undo from frame 0 goes back to frame 1 and puts it back.
        history.go_to_frame(&mut document, 0);
        history.undo(&mut document);
        assert_eq!(document.frame(), 1);
        assert_eq!(
            document.active_layer().surface.painted_area().unwrap(),
            mirrored
        );
        // A reference picture belongs to the picture: on every frame, saved once.
        let reference = crate::reference::Reference::from_bytes(
            "ref.png",
            crate::reference::tests::two_colour_png(),
        )
        .unwrap();
        let _ = crate::reference::ReferenceCommand::add(&mut document, reference);
        for frame in 0..3 {
            history.go_to_frame(&mut document, frame);
            assert_eq!(document.references().len(), 1);
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        crate::file::write_document(&document, &mut bytes).unwrap();
        bytes.set_position(0);
        assert_eq!(
            crate::file::read_document(bytes)
                .unwrap()
                .references()
                .len(),
            1
        );
    }

    /// The memory the brief worries about: 24 frames of 1024 x 1024 with
    /// several layers must not be 24 full copies.
    #[test]
    fn twenty_four_frames_cost_little_more_than_what_differs() {
        let full = |surface: &mut Surface, color: Rgba8| {
            surface.paint_pixels(PixelRect::new(0, 0, 1024, 1024), |_, _| Some(color));
        };
        let mut document = Document::new(1024, 1024);
        // A full background, still.
        full(&mut document.active_layer_mut().surface, BLUE);
        // Three animated layers, each full on the first frame.
        let mut animated = Vec::new();
        for _ in 0..3 {
            let _ = crate::layer_commands::LayerPresenceCommand::add_layer(&mut document);
            let id = document.active_id();
            full(&mut document.active_layer_mut().surface, RED);
            AnimatedCommand::set(&mut document, id, true).unwrap();
            animated.push(id);
        }
        let one_frame = document.pixel_memory();
        assert_eq!(one_frame, 4 * 16 * 256 * 256 * 4, "4 layers x 16 tiles");
        // 23 duplicates, then a stroke-sized change on each frame of one layer.
        for _ in 1..24 {
            let last = document.frame_count() - 1;
            FrameCommand::duplicate(&mut document, last).unwrap();
        }
        assert_eq!(document.pixel_memory(), one_frame, "copies cost nothing");
        for frame in 0..24 {
            document.go_to_frame(frame);
            document.set_active(animated[0]);
            document
                .active_layer_mut()
                .surface
                .set_pixel(frame as i32 * 10, 500, BLUE);
        }
        // Each frame's change copied one or two tiles, not the frame.
        let used = document.pixel_memory();
        let copies = 24 * 4 * 16 * 256 * 256 * 4;
        assert!(used <= one_frame + 24 * 2 * 256 * 256 * 4, "{used}");
        assert!(
            used * 10 < copies,
            "{used} against {copies} for 24 full copies"
        );
    }
}
