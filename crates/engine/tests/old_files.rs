//! Pictures and scripts made before the app was renamed Duck Quill must
//! keep working. The fixtures were made with the code as it was then
//! (Art App, `art-cli`): an `.artapp` picture with four layers (clip,
//! alpha lock, multiply, screen), the PNG it exported, and the script
//! that painted it, which says `"format": "artapp-script"`.
//!
//! `picture-0.5.0.quill` was saved by Duck Quill 0.5.0's code, before
//! phase 6 changed anything: the all-commands test script's eight layers
//! (one hidden, screen and multiply, 70% and 80% opacity), a clipped,
//! alpha-locked hand stroke that runs past every canvas edge (so tiles
//! with negative numbers), and a pinned reference picture. Beside it are
//! the PNGs 0.5.0 exported from it (on the paper, and see-through) and a
//! line per layer of what 0.5.0 saw in it.

use std::path::{Path, PathBuf};

use quill_engine::document::Background;
use quill_engine::file;
use quill_engine::image::Image;
use quill_engine::script::{PlayOptions, Player, Script};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn same_pixels(a: &Image, b: &Image) -> bool {
    a.width() == b.width()
        && a.height() == b.height()
        && (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)))
}

#[test]
fn an_old_artapp_picture_opens_with_the_same_pixels_and_layers() {
    let document = file::open(&fixture("old-picture.artapp")).expect("old pictures still open");
    let names: Vec<&str> = document.layers().iter().map(|l| l.name()).collect();
    assert_eq!(names, ["Ground", "Shade", "Sun", "Lines"]);
    let shade = &document.layers()[1];
    assert!(shade.clip(), "the clip setting survives");
    assert!(document.layers()[3].lock(), "the alpha lock survives");
    let expected = Image::load(&fixture("old-picture.png")).unwrap();
    let image = Image::of_document(&document, Background::Paper);
    assert!(same_pixels(&image, &expected));
}

#[test]
fn an_old_picture_saved_again_opens_again_the_same() {
    let document = file::open(&fixture("old-picture.artapp")).unwrap();
    let path =
        std::env::temp_dir().join(format!("old-picture-resaved-{}.artapp", std::process::id()));
    file::save(&document, &path).unwrap();
    let again = file::open(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(same_pixels(
        &Image::of_document(&document, Background::Paper),
        &Image::of_document(&again, Background::Paper)
    ));
}

#[test]
fn an_old_format_script_plays_with_no_warnings_and_paints_the_same() {
    let text = std::fs::read_to_string(fixture("old-script.json")).unwrap();
    assert!(
        text.contains("\"artapp-script\""),
        "the fixture must keep its old name"
    );
    let script = Script::parse(&text).expect("the old format name is accepted");
    let mut player = Player::new(script, PlayOptions::default()).unwrap();
    let mut document = player.create_document().unwrap();
    player.run(&mut document, |_, _| {}).unwrap();
    assert!(
        player.report().warnings.is_empty(),
        "{:?}",
        player.report().warnings
    );
    let expected = Image::load(&fixture("old-picture.png")).unwrap();
    assert!(same_pixels(
        &Image::of_document(&document, Background::Paper),
        &expected
    ));
}

#[test]
fn a_wrong_format_name_is_still_refused() {
    let error = Script::parse(r#"{ "format": "someone-else", "commands": [] }"#).unwrap_err();
    assert!(error.to_string().contains("duckquill-script"), "{error}");
}

/// One line per layer, as 0.5.0 wrote them into `picture-0.5.0.txt`.
fn summary(document: &quill_engine::document::Document) -> String {
    let mut summary = String::new();
    for layer in document.layers() {
        summary.push_str(&format!(
            "{} | visible {} | opacity {} | {} | clip {} | lock {} | tiles {} | area {:?}\n",
            layer.name(),
            layer.visible(),
            layer.opacity(),
            layer.blend().name(),
            layer.clip(),
            layer.lock(),
            layer.surface.tile_count(),
            layer
                .surface
                .painted_area()
                .map(|a| (a.left, a.top, a.right, a.bottom)),
        ));
    }
    summary.push_str(&format!("references {}\n", document.references().len()));
    summary
}

#[test]
fn a_picture_saved_by_0_5_0_opens_with_the_same_pixels_layers_and_reference() {
    let document = file::open(&fixture("picture-0.5.0.quill")).expect("0.5.0 pictures open");
    let expected = std::fs::read_to_string(fixture("picture-0.5.0.txt")).unwrap();
    assert_eq!(summary(&document), expected.replace("\r\n", "\n"));
    assert_eq!(document.references()[0].name, "duck-quill-icon-64.png");
    assert!(same_pixels(
        &Image::of_document(&document, Background::Paper),
        &Image::load(&fixture("picture-0.5.0.png")).unwrap()
    ));
    assert!(same_pixels(
        &Image::of_document(&document, Background::Transparent),
        &Image::load(&fixture("picture-0.5.0-transparent.png")).unwrap()
    ));
}

#[test]
fn a_0_5_0_picture_saved_again_is_the_same_picture() {
    let document = file::open(&fixture("picture-0.5.0.quill")).unwrap();
    let path = std::env::temp_dir().join(format!(
        "picture-0.5.0-resaved-{}.quill",
        std::process::id()
    ));
    file::save(&document, &path).unwrap();
    let again = file::open(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(summary(&again), summary(&document));
    assert!(same_pixels(
        &Image::of_document(&document, Background::Transparent),
        &Image::of_document(&again, Background::Transparent)
    ));
    assert_eq!(
        again.references()[0].bytes(),
        document.references()[0].bytes()
    );
}
