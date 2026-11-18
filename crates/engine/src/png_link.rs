//! A picture that lives in a PNG file, with its layers kept beside it.
//!
//! Other programs (Duck Mold, Duck Forge) hand Duck Quill a PNG to paint,
//! for example the texture of one face of a model, and watch that file:
//! when it changes, they show the new picture. So a picture opened from a
//! PNG is *linked* to it, and saving writes two files:
//!
//! - `robot_face.png`: the picture itself, flat, with its see-through
//!   parts (exactly what "Export PNG, transparent" gives; the paper is
//!   never in it);
//! - `robot_face.quill`, beside it: the same picture with its layers, so
//!   the work can carry on later.
//!
//! The PNG is the one that counts. When it's opened again, the layers
//! beside it are only used if they still show exactly the PNG's picture.
//! If another program changed the PNG meanwhile, the PNG is opened as one
//! layer instead (see [`choose`]).
//!
//! **What a round trip keeps.** A PNG stores colours the normal
//! ("straight") way and layers store them premultiplied (see `tile.rs`),
//! both in whole numbers from 0 to 255. Going there and back:
//! - a fully opaque pixel comes back exactly;
//! - a fully see-through pixel comes back fully see-through (the colour
//!   it hid, which nobody could see, becomes black);
//! - a partly see-through pixel keeps its opacity exactly, and each of
//!   its colour values moves by at most [`round_trip_error`]: 1 step
//!   from half see-through up, more the fainter the pixel is (a pixel
//!   you can barely see has little colour left to remember).

use std::path::{Path, PathBuf};

use crate::document::{Background, Document, MAX_CANVAS_SIDE};
use crate::file::{self, FILE_EXTENSION, FileError};
use crate::image::Image;
use crate::tile::Rgba8;

/// A picture counts as small when neither side is longer than this: one
/// where every pixel matters, like a texture for a model's face.
pub const SMALL_SIDE: u32 = 256;

/// True if the file's name ends in `.png` (in any mix of capitals).
pub fn is_png_name(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ending| ending.eq_ignore_ascii_case("png"))
}

/// Where a PNG's layers are kept: "robot_face.png" -> "robot_face.quill",
/// in the same folder.
pub fn layers_path(png: &Path) -> PathBuf {
    png.with_extension(FILE_EXTENSION)
}

/// True for a small picture (see [`SMALL_SIDE`]).
pub fn is_small(width: u32, height: u32) -> bool {
    width <= SMALL_SIDE && height <= SMALL_SIDE
}

/// The most a colour value (0 to 255) of a pixel with this `opacity`
/// (1 to 255) can move when a PNG is opened and saved again. The opacity
/// itself never moves.
///
/// Storing the colour premultiplied rounds it by up to half a step out of
/// `opacity` steps; turning it back stretches that to `127.5 / opacity`
/// steps out of 255, and rounds once more.
pub fn round_trip_error(opacity: u8) -> u8 {
    let opacity = u32::from(opacity.max(1));
    ((255 + opacity) / (2 * opacity)).min(255) as u8
}

/// A new picture from a flat image: the canvas is the image's size, and
/// its pixels are on the one layer, see-through where the image is.
pub fn document_from_image(image: &Image) -> Result<Document, FileError> {
    let (width, height) = (image.width(), image.height());
    if width > MAX_CANVAS_SIDE || height > MAX_CANVAS_SIDE {
        return Err(FileError::TooBig(width, height));
    }
    if width == 0 || height == 0 {
        return Err(FileError::Damaged("the picture has no pixels".into()));
    }
    let mut document = Document::new(width, height);
    let everywhere = document.bounds();
    document
        .active_layer_mut()
        .surface
        .paint_pixels(everywhere, |x, y| {
            let [r, g, b, a] = image.pixel(x as u32, y as u32);
            Some(Rgba8::from_straight(r, g, b, a))
        });
    Ok(document)
}

/// What became of the layers kept beside a PNG when it was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layers {
    /// There was no `.quill` beside the PNG: it's one layer.
    None,
    /// The `.quill` showed exactly the PNG's picture, so it was opened:
    /// the layers are all there.
    Kept,
    /// The `.quill` showed another picture (the PNG was changed since the
    /// layers were saved), so the PNG was opened as one layer.
    Outdated,
    /// The `.quill` couldn't be opened, so the PNG was opened as one layer.
    Unreadable,
}

/// A picture opened from a PNG.
#[derive(Debug)]
pub struct Opened {
    pub document: Document,
    pub layers: Layers,
}

/// Decides what to open for a PNG: its saved layers, or the PNG itself.
///
/// `png` is the PNG's picture. `saved` is what opening the `.quill`
/// beside it gave (`None` if there isn't one).
///
/// The layers win only if they show exactly the PNG's picture: the same
/// size and the same pixels with the paper left out (two fully
/// see-through pixels count as the same, whatever colour they hide).
/// Otherwise another program changed the PNG after the layers were
/// saved, and the PNG wins: it's opened as one layer.
pub fn choose(
    png: &Image,
    saved: Option<Result<Document, FileError>>,
) -> Result<Opened, FileError> {
    let layers = match saved {
        None => Layers::None,
        Some(Err(_)) => Layers::Unreadable,
        Some(Ok(document)) => {
            if shows(&document, png) {
                return Ok(Opened {
                    document,
                    layers: Layers::Kept,
                });
            }
            Layers::Outdated
        }
    };
    Ok(Opened {
        document: document_from_image(png)?,
        layers,
    })
}

/// True if `document`, with the paper left out, is exactly `picture`.
fn shows(document: &Document, picture: &Image) -> bool {
    // (The size first: no need to flatten the layers if it differs.)
    (document.width(), document.height()) == (picture.width(), picture.height())
        && Image::of_document(document, Background::Transparent).same_picture_as(picture)
}

/// Opens the PNG at `png`, with the layers beside it if they still show
/// the same picture (see [`choose`]).
pub fn open(png: &Path) -> Result<Opened, FileError> {
    let picture = Image::load(png)?;
    let beside = layers_path(png);
    let saved = beside.is_file().then(|| file::open(&beside));
    choose(&picture, saved)
}

/// Saving a linked picture failed: which of the two files, and why.
#[derive(Debug)]
pub struct SaveError {
    pub path: PathBuf,
    pub error: FileError,
}

/// Saves a picture linked to the PNG at `png`: first its layers in the
/// `.quill` beside it, then the PNG itself (as a transparent export), so
/// the PNG is the last thing to change and a program watching it sees
/// the finished picture. Returns where the layers went.
///
/// Each file is written under a temporary name and then swapped in (see
/// `file::save`), so a failure never leaves half a PNG: the PNG is
/// either the old picture or the new one.
pub fn save(document: &Document, png: &Path) -> Result<PathBuf, SaveError> {
    let layers = layers_path(png);
    file::save(document, &layers).map_err(|error| SaveError {
        path: layers.clone(),
        error,
    })?;
    file::export_png(document, png, Background::Transparent).map_err(|error| SaveError {
        path: png.to_path_buf(),
        error,
    })?;
    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Color;
    use crate::layer_commands::LayerPresenceCommand;
    use std::fs;

    /// An empty folder of this test's own, in the system's temporary folder.
    fn test_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("quill-png-link-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn png_bytes(image: &Image) -> Vec<u8> {
        let mut bytes = Vec::new();
        image.write_png(&mut bytes, true).unwrap();
        bytes
    }

    /// `image` as a PNG file's bytes, opened as a picture, and saved as
    /// PNG bytes again: what comes back when those are read.
    fn there_and_back(image: &Image) -> Image {
        let read = Image::decode(&png_bytes(image)).unwrap();
        let document = document_from_image(&read).unwrap();
        let mut bytes = Vec::new();
        file::write_png(&document, Background::Transparent, &mut bytes).unwrap();
        Image::decode(&bytes).unwrap()
    }

    /// A picture bigger than one tile each way, with a different colour
    /// in every pixel, each as opaque as `opacity` says.
    fn colourful(opacity: impl Fn(u32, u32) -> u8) -> Image {
        let mut image = Image::new(300, 270, [0, 0, 0, 0]);
        for y in 0..270 {
            for x in 0..300 {
                let color = [
                    (x % 256) as u8,
                    (y % 256) as u8,
                    ((x * 7 + y * 13) % 256) as u8,
                ];
                let [r, g, b] = color;
                image.set_pixel(x as i32, y as i32, [r, g, b, opacity(x, y)]);
            }
        }
        image
    }

    #[test]
    fn a_fully_opaque_png_comes_back_exactly() {
        let picture = colourful(|_, _| 255);
        assert_eq!(there_and_back(&picture), picture);
    }

    #[test]
    fn a_fully_see_through_png_comes_back_fully_see_through() {
        // Colours hidden in see-through pixels, as some programs leave them.
        let picture = colourful(|_, _| 0);
        let back = there_and_back(&picture);
        assert_eq!(back, Image::new(300, 270, [0, 0, 0, 0]));
        assert!(back.same_picture_as(&picture));
        // Nothing was painted, so the picture holds no tiles at all.
        let document = document_from_image(&picture).unwrap();
        assert_eq!(document.tile_count(), 0);
        assert_eq!((document.width(), document.height()), (300, 270));
    }

    #[test]
    fn opaque_and_see_through_pixels_side_by_side_come_back_exactly() {
        // A sprite: solid in a checker pattern, nothing in between.
        let picture = colourful(|x, y| if (x / 3 + y / 5) % 2 == 0 { 255 } else { 0 });
        let back = there_and_back(&picture);
        assert!(back.same_picture_as(&picture));
        for y in 0..270 {
            for x in 0..300 {
                let before = picture.pixel(x, y);
                let expected: Color = if before[3] == 255 {
                    before
                } else {
                    [0, 0, 0, 0]
                };
                assert_eq!(back.pixel(x, y), expected, "at {x}, {y}");
            }
        }
    }

    #[test]
    fn partly_see_through_pixels_come_back_within_the_rounding() {
        // Every colour value at every opacity from 1 to 254.
        let mut picture = Image::new(256, 254, [0, 0, 0, 0]);
        for opacity in 1..=254u8 {
            for value in 0..=255u8 {
                let color = [value, 255 - value, value / 2, opacity];
                picture.set_pixel(i32::from(value), i32::from(opacity) - 1, color);
            }
        }
        let back = there_and_back(&picture);
        let mut worst = 0;
        for opacity in 1..=254u8 {
            let allowed = i32::from(round_trip_error(opacity));
            for value in 0..=255u32 {
                let before = picture.pixel(value, u32::from(opacity) - 1);
                let after = back.pixel(value, u32::from(opacity) - 1);
                assert_eq!(after[3], before[3], "the opacity never moves");
                for channel in 0..3 {
                    let moved = (i32::from(after[channel]) - i32::from(before[channel])).abs();
                    assert!(
                        moved <= allowed,
                        "{before:?} came back as {after:?}: more than {allowed} off"
                    );
                    if opacity >= 128 {
                        worst = worst.max(moved);
                    }
                }
            }
        }
        // From half see-through up, never more than one step.
        assert_eq!(worst, 1);
        assert_eq!(round_trip_error(128), 1);
        assert_eq!(round_trip_error(254), 1);
        // The fainter, the more: 2 steps at a quarter of full opacity,
        // 13 for a pixel only a twenty-fifth there, and half the range
        // for one you can hardly see at all.
        assert_eq!(round_trip_error(64), 2);
        assert_eq!(round_trip_error(10), 13);
        assert_eq!(round_trip_error(1), 128);
        // Saving a second time changes nothing more.
        assert_eq!(there_and_back(&back), back);
    }

    /// A small picture with two layers: red at the bottom, half
    /// see-through blue on top, and nothing in one corner.
    fn two_layers() -> Document {
        let mut document = Document::new(16, 12);
        let everywhere = document.bounds();
        document
            .active_layer_mut()
            .surface
            .paint_pixels(everywhere, |x, y| {
                (x > 2 || y > 2).then_some(Rgba8::new(255, 0, 0, 255))
            });
        LayerPresenceCommand::add_layer(&mut document);
        document
            .active_layer_mut()
            .surface
            .set_pixel(8, 6, Rgba8::from_straight(0, 0, 255, 128));
        document
    }

    #[test]
    fn a_linked_save_writes_the_png_and_its_layers() {
        let folder = test_folder("save");
        let png = folder.join("robot_face.png");
        // (With a paper colour, to see that it stays out of the PNG.)
        let mut document = two_layers();
        document.set_paper(Rgba8::new(40, 90, 200, 255));

        let layers = save(&document, &png).unwrap();
        assert_eq!(layers, folder.join("robot_face.quill"));
        assert!(png.is_file() && layers.is_file());
        // Nothing temporary is left behind.
        let mut names: Vec<String> = fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["robot_face.png", "robot_face.quill"]);

        // The PNG is the transparent export, to the byte.
        let mut export = Vec::new();
        file::write_png(&document, Background::Transparent, &mut export).unwrap();
        assert_eq!(fs::read(&png).unwrap(), export);
        let picture = Image::load(&png).unwrap();
        assert_eq!(
            picture,
            Image::of_document(&document, Background::Transparent)
        );
        assert_eq!(picture.pixel(0, 0), [0, 0, 0, 0], "no paper in the PNG");
        assert_eq!(picture.pixel(5, 5), [255, 0, 0, 255]);

        // The layers are all in the .quill, with the paper.
        let saved = file::open(&layers).unwrap();
        assert_eq!(saved.layers().len(), 2);
        assert_eq!(saved.paper(), Rgba8::new(40, 90, 200, 255));

        // Opening the PNG again gives the layers back.
        let opened = open(&png).unwrap();
        assert_eq!(opened.layers, Layers::Kept);
        assert_eq!(opened.document.layers().len(), 2);
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_png_alone_opens_as_one_layer() {
        let folder = test_folder("alone");
        let png = folder.join("leaf.PNG");
        let picture = Image::of_document(&two_layers(), Background::Transparent);
        fs::write(&png, png_bytes(&picture)).unwrap();
        let opened = open(&png).unwrap();
        assert_eq!(opened.layers, Layers::None);
        assert_eq!(opened.document.layers().len(), 1);
        assert_eq!(
            (opened.document.width(), opened.document.height()),
            (16, 12)
        );
        assert_eq!(
            Image::of_document(&opened.document, Background::Transparent),
            picture
        );
        // A file that isn't there, or isn't a picture, is an error, not a crash.
        assert!(matches!(
            open(&folder.join("missing.png")),
            Err(FileError::Io(_))
        ));
        fs::write(folder.join("words.png"), "not a picture").unwrap();
        assert!(matches!(
            open(&folder.join("words.png")),
            Err(FileError::Damaged(_))
        ));
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn the_layers_win_only_if_they_show_the_same_picture() {
        let saved = two_layers();
        let same = Image::of_document(&saved, Background::Transparent);

        // The same pixels: the layers open.
        let opened = choose(&same, Some(Ok(saved.clone()))).unwrap();
        assert_eq!(opened.layers, Layers::Kept);
        assert_eq!(opened.document.layers().len(), 2);

        // Another colour hidden in a see-through pixel is still the same picture.
        let mut hidden = same.clone();
        assert_eq!(hidden.pixel(0, 0)[3], 0);
        hidden.set_pixel(0, 0, [255, 255, 255, 0]);
        let opened = choose(&hidden, Some(Ok(saved.clone()))).unwrap();
        assert_eq!(opened.layers, Layers::Kept);

        // One pixel changed by another program: the PNG wins, as one layer.
        let mut changed = same.clone();
        changed.set_pixel(4, 4, [0, 200, 0, 255]);
        let opened = choose(&changed, Some(Ok(saved.clone()))).unwrap();
        assert_eq!(opened.layers, Layers::Outdated);
        assert_eq!(opened.document.layers().len(), 1);
        assert_eq!(
            Image::of_document(&opened.document, Background::Transparent),
            changed
        );

        // Another size: the PNG wins.
        let bigger = Image::new(32, 12, [255, 0, 0, 255]);
        let opened = choose(&bigger, Some(Ok(saved.clone()))).unwrap();
        assert_eq!(opened.layers, Layers::Outdated);
        assert_eq!(
            (opened.document.width(), opened.document.height()),
            (32, 12)
        );
        // Even with the same number of pixels.
        let turned = Image::new(12, 16, [255, 0, 0, 255]);
        let opened = choose(&turned, Some(Ok(saved))).unwrap();
        assert_eq!(opened.layers, Layers::Outdated);

        // No layers beside it, or layers that can't be read: the PNG, as one layer.
        let opened = choose(&same, None).unwrap();
        assert_eq!(opened.layers, Layers::None);
        assert_eq!(opened.document.layers().len(), 1);
        let opened = choose(&same, Some(Err(FileError::NotDuckQuill))).unwrap();
        assert_eq!(opened.layers, Layers::Unreadable);
        assert_eq!(opened.document.layers().len(), 1);
    }

    #[test]
    fn a_png_changed_by_another_program_wins_and_the_old_layers_stay_until_saved() {
        let folder = test_folder("changed");
        let png = folder.join("door.png");
        save(&two_layers(), &png).unwrap();

        // Another program paints the PNG green.
        let green = Image::new(16, 12, [0, 160, 0, 255]);
        fs::write(&png, png_bytes(&green)).unwrap();
        let opened = open(&png).unwrap();
        assert_eq!(opened.layers, Layers::Outdated);
        assert_eq!(opened.document.layers().len(), 1);
        assert_eq!(
            Image::of_document(&opened.document, Background::Transparent),
            green
        );
        // The old layers are still in the .quill...
        assert_eq!(file::open(&layers_path(&png)).unwrap().layers().len(), 2);
        // ...until the picture is saved: then they're the PNG's again.
        save(&opened.document, &png).unwrap();
        assert_eq!(file::open(&layers_path(&png)).unwrap().layers().len(), 1);
        assert_eq!(open(&png).unwrap().layers, Layers::Kept);

        // Layers that can't be read don't stop the PNG from opening.
        fs::write(layers_path(&png), "scribble").unwrap();
        assert_eq!(open(&png).unwrap().layers, Layers::Unreadable);
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_failed_save_leaves_the_png_whole() {
        let folder = test_folder("failed");
        let png = folder.join("wall.png");
        let before = png_bytes(&Image::new(16, 12, [9, 9, 9, 255]));
        fs::write(&png, &before).unwrap();

        // The layers can't be written (a folder is in the way): the PNG
        // isn't touched, because the layers are written first.
        fs::create_dir(folder.join("wall.quill")).unwrap();
        let failed = save(&two_layers(), &png).unwrap_err();
        assert_eq!(failed.path, folder.join("wall.quill"));
        assert_eq!(fs::read(&png).unwrap(), before);
        fs::remove_dir(folder.join("wall.quill")).unwrap();

        // The PNG can't be written (its temporary name is taken by a
        // folder): the old PNG is still there, whole.
        fs::create_dir(folder.join("wall.png.saving")).unwrap();
        let failed = save(&two_layers(), &png).unwrap_err();
        assert_eq!(failed.path, png);
        assert!(matches!(failed.error, FileError::Io(_)));
        assert_eq!(fs::read(&png).unwrap(), before);
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn names_sizes_and_limits() {
        assert!(is_png_name(Path::new("a.png")));
        assert!(is_png_name(Path::new("a.b.PNG")));
        assert!(!is_png_name(Path::new("a.quill")));
        assert!(!is_png_name(Path::new("png")));
        let folder = Path::new("faces");
        assert_eq!(
            layers_path(&folder.join("robot.face.png")),
            folder.join("robot.face.quill")
        );
        assert!(is_small(256, 256) && is_small(1, 200));
        assert!(!is_small(257, 16) && !is_small(16, 257));
        // A picture too wide for any canvas is turned away with its size.
        let wide = Image::new(MAX_CANVAS_SIDE + 1, 1, [0, 0, 0, 255]);
        let error = document_from_image(&wide).unwrap_err();
        assert!(matches!(error, FileError::TooBig(_, 1)), "{error}");
        assert!(error.to_string().contains("50001 × 1"), "{error}");
    }
}
