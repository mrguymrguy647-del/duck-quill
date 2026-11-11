//! Pictures saved by Duck Quill 0.6.0, kept so later versions keep
//! opening them, and so the real 0.5.0 can be tried on them (CI builds
//! 0.5.0 and runs `.github/compat/old-reader.sh`):
//!
//! - `plain-0.6.0.quill`: no frames. Saved as file version 1, exactly as
//!   0.5.0 saved pictures, so 0.5.0 opens it with the same pixels.
//! - `walk-0.6.0.quill`: four frames, a still background, an animated
//!   layer, a hold and a named stretch. File version 2: 0.5.0 must say
//!   it's from a newer version, never open part of it.
//!
//! Made again with `QUILL_MAKE_FIXTURES=1 cargo test -p quill_engine
//! --test new_files` (then run the old reader script to update its
//! expected output, and say why in the commit).

use std::path::{Path, PathBuf};

use quill_engine::document::{Background, Document};
use quill_engine::file;
use quill_engine::frames::{AnimatedCommand, FrameCommand, Stretch, TimelineCommand};
use quill_engine::image::Image;
use quill_engine::layer_commands::LayerPresenceCommand;
use quill_engine::surface::PixelRect;
use quill_engine::tile::Rgba8;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn square(document: &mut Document, x: i32, y: i32, size: i32, color: Rgba8) {
    document
        .active_layer_mut()
        .surface
        .paint_pixels(PixelRect::new(x, y, x + size, y + size), |_, _| Some(color));
}

fn plain() -> Document {
    let mut document = Document::new(320, 200);
    square(&mut document, 20, 20, 120, Rgba8::new(40, 120, 200, 255));
    let _ = LayerPresenceCommand::add_layer(&mut document);
    square(&mut document, 200, 60, 300, Rgba8::new(200, 60, 40, 255)); // past the edge
    document
}

fn walk() -> Document {
    let mut document = Document::new(320, 200);
    square(&mut document, 0, 150, 320, Rgba8::new(90, 160, 70, 255)); // still ground
    let _ = LayerPresenceCommand::add_layer(&mut document);
    let duck = document.active_id();
    AnimatedCommand::set(&mut document, duck, true).unwrap();
    for _ in 1..4 {
        let end = document.frame_count();
        FrameCommand::add(&mut document, end).unwrap();
    }
    for frame in 0..4 {
        document.go_to_frame(frame);
        square(
            &mut document,
            40 + 60 * frame as i32,
            100,
            40,
            Rgba8::new(240, 200, 40, 255),
        );
    }
    FrameCommand::hold(&mut document, 3, 2).unwrap();
    let mut settings = document.timeline().settings();
    settings.stretches = vec![Stretch {
        name: "walk".into(),
        start: 0,
        end: 4,
        fps: 8.0,
        looping: true,
    }];
    TimelineCommand::apply(&mut document, settings).unwrap();
    document.go_to_frame(1);
    document
}

fn same(a: &Image, b: &Image) -> bool {
    a.width() == b.width()
        && a.height() == b.height()
        && (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)))
}

#[test]
fn make_the_fixtures_when_asked() {
    if std::env::var_os("QUILL_MAKE_FIXTURES").is_none() {
        return;
    }
    let plain = plain();
    file::save(&plain, &fixture("plain-0.6.0.quill")).unwrap();
    Image::of_document(&plain, Background::Paper)
        .save_png(&fixture("plain-0.6.0.png"), false)
        .unwrap();
    let walk = walk();
    file::save(&walk, &fixture("walk-0.6.0.quill")).unwrap();
    for frame in 0..walk.frame_count() {
        Image::of_document(&walk.frame_picture(frame), Background::Paper)
            .save_png(&fixture(&format!("walk-0.6.0-{frame}.png")), false)
            .unwrap();
    }
}

#[test]
fn the_0_6_0_pictures_open_with_every_frame() {
    let opened = file::open(&fixture("plain-0.6.0.quill")).unwrap();
    assert!(!opened.is_animated());
    assert!(same(
        &Image::of_document(&opened, Background::Paper),
        &Image::load(&fixture("plain-0.6.0.png")).unwrap()
    ));
    let opened = file::open(&fixture("walk-0.6.0.quill")).unwrap();
    assert_eq!((opened.frame_count(), opened.frame()), (4, 1));
    assert_eq!(opened.timeline().frames()[3].hold, 2);
    assert_eq!(opened.timeline().stretches[0].name, "walk");
    for frame in 0..4 {
        assert!(
            same(
                &Image::of_document(&opened.frame_picture(frame), Background::Paper),
                &Image::load(&fixture(&format!("walk-0.6.0-{frame}.png"))).unwrap()
            ),
            "frame {frame}"
        );
    }
}

#[test]
fn the_fixtures_are_what_this_code_makes() {
    // If saving changes, the fixtures (and what 0.5.0 was tried on) must
    // be made again on purpose.
    for (name, document) in [
        ("plain-0.6.0.png", plain()),
        ("walk-0.6.0-0.png", walk().frame_picture(0)),
    ] {
        assert!(
            same(
                &Image::of_document(&document, Background::Paper),
                &Image::load(&fixture(name)).unwrap()
            ),
            "{name}"
        );
    }
}
