//! Baking: a script's animations turned into a picture with frames, to
//! draw on by hand ("Edit frames by hand" in the app, `quill bake`).
//!
//! Every frame of every animation is painted, in order, into one
//! timeline; each animation becomes a named stretch with its fps and
//! loop, and the script's pivot is the picture's. Layers are matched by
//! name across all frames: a layer that's the same on every frame is
//! still, any other is animated. The script itself is never changed.

use std::path::PathBuf;

use super::{PlayOptions, Player, Script, ScriptError, Sequence};
use crate::document::Document;
use crate::frames::{FrameInfo, Stretch, Timeline};
use crate::layer::{Layer, LayerId, LayerProps};
use crate::surface::Surface;

/// Paints every frame of every animation of `script` (`folder` is the
/// script's own, for `include` and `open`) into one picture with frames.
/// A script with no animations gives a picture of one frame.
pub fn bake(script: &Script, folder: Option<PathBuf>) -> Result<Document, ScriptError> {
    // Every frame, as a picture, and the stretches they make.
    let mut pictures: Vec<Document> = Vec::new();
    let mut stretches = Vec::new();
    if script.animations.is_empty() {
        let mut player = Player::new(
            script.clone(),
            PlayOptions {
                folder,
                ..PlayOptions::default()
            },
        )?;
        let mut document = player.create_document()?;
        player.run(&mut document, |_, _| {})?;
        pictures.push(document);
    } else {
        for (name, animation) in &script.animations {
            let start = pictures.len();
            let mut sequence = Sequence::new(
                script.clone(),
                PlayOptions {
                    folder: folder.clone(),
                    animation: Some(name.clone()),
                    ..PlayOptions::default()
                },
            );
            for frame in 0..animation.frames {
                pictures.push(sequence.paint(frame, |_, _| {})?.0);
            }
            stretches.push(Stretch {
                name: name.clone(),
                start,
                end: pictures.len(),
                fps: animation.fps.clamp(1.0, 60.0),
                looping: animation.looping,
            });
        }
    }
    if pictures.len() > crate::frames::MAX_FRAMES {
        return Err(ScriptError::general(format!(
            "{} frames is more than a picture can hold ({})",
            pictures.len(),
            crate::frames::MAX_FRAMES
        )));
    }

    // The layers by name, in the order they first appear.
    let mut names: Vec<(String, LayerProps)> = Vec::new();
    for picture in &pictures {
        for layer in picture.layers() {
            if !names.iter().any(|(name, _)| name == layer.name()) {
                names.push((layer.name().to_owned(), layer.props()));
            }
        }
    }
    let first = &pictures[0];
    let mut layers = Vec::with_capacity(names.len());
    for (index, (name, props)) in names.into_iter().enumerate() {
        let drawings: Vec<Surface> = pictures
            .iter()
            .map(|picture| {
                picture
                    .layers()
                    .iter()
                    .find(|layer| layer.name() == name)
                    .map(|layer| {
                        let mut surface = layer.surface.clone();
                        surface.locked = false;
                        let _ = surface.take_dirty_tiles();
                        let _ = surface.take_thumbnail_tiles();
                        surface
                    })
                    .unwrap_or_default()
            })
            .collect();
        let still = drawings
            .windows(2)
            .all(|pair| same_pixels(&pair[0], &pair[1]));
        let mut drawings = drawings;
        let shown = std::mem::take(&mut drawings[0]);
        let mut layer = Layer::from_saved(LayerId(index as u64 + 1), props, shown);
        if !still {
            layer.drawings = Some(drawings);
        }
        layers.push(layer);
    }
    let active = first.layer_index(first.active_id()).unwrap_or(0);
    let count = layers.len() as u64;
    let mut document = Document::from_parts(
        first.width(),
        first.height(),
        first.paper(),
        layers,
        active,
        count + 1,
    );
    let (fps, looping) = stretches
        .first()
        .map_or((12.0, true), |s| (s.fps, s.looping));
    document.timeline = Timeline {
        frames: vec![FrameInfo::default(); pictures.len()],
        current: 0,
        fps,
        looping,
        // One animation is the whole timeline: no need to name it.
        stretches: if stretches.len() > 1 {
            stretches
        } else {
            Vec::new()
        },
        pivot: script.pivot,
    };
    Ok(document)
}

/// True if two drawings have exactly the same pixels.
fn same_pixels(a: &Surface, b: &Surface) -> bool {
    if a.tile_count() != b.tile_count() {
        return false;
    }
    a.tiles_shared().all(|(coord, tile)| {
        b.tile_shared(coord)
            .is_some_and(|other| std::sync::Arc::ptr_eq(tile, &other) || **tile == *other)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Background;
    use crate::image::Image;

    const SCRIPT: &str = r##"{ "palette": { "ink": "#202020", "sky": "#8EC5E8" }, "pivot": [10, 18],
      "commands": [
        { "cmd": "canvas", "width": 20, "height": 20 },
        { "cmd": "layer", "name": "Sky" },
        { "cmd": "rect", "x": 0, "y": 0, "width": 20, "height": 10, "fill": "sky" },
        { "cmd": "layer", "name": "Dot" },
        { "cmd": "define", "name": "dot", "commands": [
            { "cmd": "rect", "x": 0, "y": 0, "width": 4, "height": 4, "fill": "ink" } ] }
      ], "animations": {
        "idle": { "frames": 3, "fps": 6, "loop": true, "commands": [
            { "cmd": "use", "name": "dot", "translate": { "keys": [[0, [2, 12]], [2, [12, 12]]] } } ] },
        "hop": { "frames": 2, "fps": 12, "loop": false, "commands": [
            { "cmd": "use", "name": "dot", "translate": [6, 2] } ] }
      } }"##;

    #[test]
    fn every_frame_of_every_animation_becomes_a_frame_in_its_stretch() {
        let script = Script::parse(SCRIPT).unwrap();
        let document = bake(&script, None).unwrap();
        assert_eq!(document.frame_count(), 5);
        let names: Vec<(&str, usize, usize, f32, bool)> = document
            .timeline()
            .stretches
            .iter()
            .map(|s| (s.name.as_str(), s.start, s.end, s.fps, s.looping))
            .collect();
        assert_eq!(
            names,
            [("idle", 0, 3, 6.0, true), ("hop", 3, 5, 12.0, false)]
        );
        assert_eq!(document.timeline().pivot, Some([10.0, 18.0]));
        // The sky never changes: still. The dot moves: animated.
        let layers: Vec<(&str, bool)> = document
            .layers()
            .iter()
            .map(|l| (l.name(), l.animated()))
            .collect();
        assert_eq!(layers, [("Sky", false), ("Dot", true)]);
        // Each frame is exactly what the script paints for it.
        for (animation, frame, baked) in [("idle", 0, 0), ("idle", 2, 2), ("hop", 1, 4)] {
            let mut player = Player::new(
                script.clone(),
                PlayOptions {
                    animation: Some(animation.into()),
                    frame,
                    ..PlayOptions::default()
                },
            )
            .unwrap();
            let mut painted = player.create_document().unwrap();
            player.run(&mut painted, |_, _| {}).unwrap();
            let a = Image::of_document(&painted, Background::Paper);
            let b = Image::of_document(&document.frame_picture(baked), Background::Paper);
            let same = (0..20).all(|y| (0..20).all(|x| a.pixel(x, y) == b.pixel(x, y)));
            assert!(same, "{animation} frame {frame}");
        }
    }

    #[test]
    fn a_script_without_animations_bakes_to_one_frame() {
        let script = Script::parse(
            r##"{ "commands": [ { "cmd": "canvas", "width": 8, "height": 8 },
                { "cmd": "rect", "x": 0, "y": 0, "width": 4, "height": 4, "fill": "#FF0000" } ] }"##,
        )
        .unwrap();
        let document = bake(&script, None).unwrap();
        assert_eq!(document.frame_count(), 1);
        assert!(!document.is_animated());
        assert_eq!(document.active_layer().surface.pixel(1, 1).a, 255);
    }
}
