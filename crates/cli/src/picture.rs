//! `quill` on pictures (`.quill`), above all hand-drawn animations made
//! in the app: `check`, `render`, `critique` and `export` read them, and
//! `apply` and `bake` make and change them. This is where an agent meets
//! the owner's drawings: it can look at every frame, improve one with a
//! small script, and export the sheet Duck Forge reads.
//!
//! Frames are counted from 0, as in scripts and the sprite sheet's JSON
//! (the app's timeline shows them from 1: frame 3 here is the 4th there).

use std::path::{Path, PathBuf};
use std::time::Instant;

use quill_engine::document::{Background, Document};
use quill_engine::file;
use quill_engine::gif::save_gif;
use quill_engine::image::Image;
use quill_engine::layer::BlendMode;
use quill_engine::layer::blend_over;
use quill_engine::script::{PlayOptions, Player};
use quill_engine::surface::PixelRect;
use quill_engine::tile::Rgba8;

use crate::sheet::frame_sheet;
use crate::{Problem, failed, load};

/// True if `path` is a picture (`.quill`, or `.artapp` from before).
pub fn is_picture(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case(file::FILE_EXTENSION)
            || e.eq_ignore_ascii_case(file::OLD_FILE_EXTENSION)
    })
}

/// The picture among the arguments, wherever it is (options may come
/// first: `quill render --frame 2 walk.quill`).
pub fn picture_argument(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .find(|p| is_picture(p))
}

fn open(path: &Path) -> Result<Document, Problem> {
    file::open(path).map_err(|e| failed(format!("{}: {e}", path.display())))
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or_else(
        || "picture".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// The options these commands share.
struct Options {
    path: PathBuf,
    out: PathBuf,
    frame: Option<usize>,
    frames: Option<(usize, usize)>,
    all_frames: bool,
    frame_sheet: bool,
    onion: bool,
    transparent: bool,
    sheet: bool,
    gif: bool,
    mp4: bool,
    export_frames: bool,
    scale: u32,
    detail: bool,
}

fn parse(args: &[String], allowed: &[&str]) -> Result<Options, Problem> {
    let mut options = Options {
        path: PathBuf::new(),
        out: PathBuf::from("out"),
        frame: None,
        frames: None,
        all_frames: false,
        frame_sheet: false,
        onion: false,
        transparent: false,
        sheet: false,
        gif: false,
        mp4: false,
        export_frames: false,
        scale: 1,
        detail: false,
    };
    let mut path = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg.starts_with("--") && !allowed.contains(&arg.as_str()) {
            return Err(Problem::Usage(format!(
                "{arg} isn't an option for a picture (.quill); these are: {}",
                allowed.join(", ")
            )));
        }
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or_else(|| Problem::Usage(format!("{name} needs a value")))
        };
        let number = |name: &str, text: String| {
            text.parse::<usize>()
                .map_err(|_| Problem::Usage(format!("{name} needs a whole number, not {text}")))
        };
        match arg.as_str() {
            "--out" => options.out = PathBuf::from(value("--out")?),
            "--frame" => options.frame = Some(number("--frame", value("--frame")?)?),
            "--frames" => {
                let text = value("--frames")?;
                let (a, b) = text.split_once('-').ok_or_else(|| {
                    Problem::Usage(format!("--frames is a range like 0-7, not {text}"))
                })?;
                let (first, last) = (number("--frames", a.into())?, number("--frames", b.into())?);
                if first > last {
                    return Err(Problem::Usage(format!(
                        "--frames goes from the first to the last, like 0-7: not {text}"
                    )));
                }
                options.frames = Some((first, last));
            }
            "--all-frames" => options.all_frames = true,
            "--frame-sheet" => options.frame_sheet = true,
            "--onion" => options.onion = true,
            "--transparent" => options.transparent = true,
            "--sheet" => options.sheet = true,
            "--gif" => options.gif = true,
            "--mp4" => options.mp4 = true,
            "--frames-png" => options.export_frames = true,
            "--detail" => options.detail = true,
            "--scale" => {
                let text = value("--scale")?;
                options.scale = text
                    .parse::<u32>()
                    .ok()
                    .filter(|s| (1..=32).contains(s))
                    .ok_or_else(|| Problem::Usage(format!("--scale is 1 to 32, not {text}")))?;
            }
            other if path.is_none() => path = Some(PathBuf::from(other)),
            other => {
                return Err(Problem::Usage(format!(
                    "only one picture at a time (also got {other})"
                )));
            }
        }
    }
    options.path = path.ok_or_else(|| Problem::Usage("which picture?".into()))?;
    Ok(options)
}

/// Frame `n` exists, or says what does.
fn check_frame(document: &Document, n: usize) -> Result<usize, Problem> {
    let count = document.frame_count();
    if n < count {
        Ok(n)
    } else {
        Err(failed(format!(
            "the picture has {count} frame{} (0 to {}), so there's no frame {n}",
            if count == 1 { "" } else { "s" },
            count - 1
        )))
    }
}

/// The box around what the animated layers draw on frame `frame` (all
/// layers when none is animated).
fn drawing_box(document: &Document, frame: usize) -> Option<PixelRect> {
    let current = document.frame();
    let any_animated = document.layers().iter().any(|l| l.animated());
    document
        .layers()
        .iter()
        .filter(|l| l.visible() && (l.animated() || !any_animated))
        .filter_map(|l| l.drawing(frame, current).painted_area())
        .reduce(|a, b| a.union(&b))
}

fn middle(area: PixelRect) -> (f32, f32) {
    (
        (area.left + area.right) as f32 / 2.0,
        (area.top + area.bottom) as f32 / 2.0,
    )
}

/// One line about the picture's frames, for `info` and `check`.
pub fn frames_summary(document: &Document) -> Vec<String> {
    let timeline = document.timeline();
    let count = document.frame_count();
    let mut lines = Vec::new();
    let held: Vec<String> = timeline
        .frames()
        .iter()
        .enumerate()
        .filter(|(_, f)| f.hold > 1)
        .map(|(i, f)| format!("frame {i} x{}", f.hold))
        .collect();
    lines.push(format!(
        "{count} frame{} (from 0; the app shows them from 1), frame {} shown, {} fps{}{}",
        if count == 1 { "" } else { "s" },
        document.frame(),
        timeline.fps,
        if timeline.looping { ", loops" } else { "" },
        if held.is_empty() {
            String::new()
        } else {
            format!("; held: {}", held.join(", "))
        }
    ));
    if !timeline.stretches.is_empty() {
        let list: Vec<String> = timeline
            .stretches
            .iter()
            .map(|s| {
                format!(
                    "{} frames {}-{} ({} fps{})",
                    s.name,
                    s.start,
                    s.end - 1,
                    s.fps,
                    if s.looping { ", loops" } else { "" }
                )
            })
            .collect();
        lines.push(format!("stretches: {}", list.join("; ")));
    }
    let layers: Vec<String> = document
        .layers()
        .iter()
        .map(|l| {
            format!(
                "{} {}",
                l.name(),
                if l.animated() { "animated" } else { "still" }
            )
        })
        .collect();
    lines.push(format!("layers: {}", layers.join(", ")));
    let [x, y] = document.pivot();
    lines.push(format!(
        "pivot ({x}, {y}){}",
        if timeline.pivot.is_none() {
            " (the bottom middle)"
        } else {
            ""
        }
    ));
    lines
}

/// `quill check picture.quill`: every frame's drawing box, how far it
/// moves, and what looks like a mistake.
pub fn check(args: &[String]) -> Result<(), Problem> {
    let options = parse(args, &["--detail"])?;
    let started = Instant::now();
    let document = open(&options.path)?;
    let count = document.frame_count();
    let mut warnings = Vec::new();
    let boxes: Vec<Option<PixelRect>> = (0..count).map(|f| drawing_box(&document, f)).collect();
    let canvas = PixelRect::new(0, 0, document.width() as i32, document.height() as i32);
    for (frame, area) in boxes.iter().enumerate() {
        match area {
            None if count > 1 => {
                warnings.push(format!("frame {frame} is empty (nothing drawn on it)"))
            }
            Some(a) if a.intersect(&canvas).is_empty() => {
                warnings.push(format!("frame {frame} draws entirely outside the canvas"));
            }
            _ => {}
        }
    }
    // Movement between frames, in each animation.
    let quarter = document.width().min(document.height()) as f32 / 4.0;
    let mut motion = Vec::new();
    for played in document.animations(&stem(&options.path)) {
        let mut steps = Vec::new();
        for pair in played.frames.windows(2) {
            if let (Some(a), Some(b)) = (boxes[pair[0]], boxes[pair[1]]) {
                let (ma, mb) = (middle(a), middle(b));
                let step = ((mb.0 - ma.0).powi(2) + (mb.1 - ma.1).powi(2)).sqrt();
                if step > quarter {
                    warnings.push(format!(
                        "\"{}\": frame {} to {} jumps {step:.0} px (more than a quarter of the frame)",
                        played.name, pair[0], pair[1]
                    ));
                }
                steps.push((pair[0], pair[1], step));
            }
        }
        let biggest = steps
            .iter()
            .cloned()
            .fold(None, |best: Option<(usize, usize, f32)>, s| {
                if best.is_none_or(|b| s.2 > b.2) {
                    Some(s)
                } else {
                    best
                }
            });
        let mut line = format!("\"{}\": {} frames", played.name, played.frames.len());
        if let Some((a, b, step)) = biggest {
            line += &format!(", biggest step {step:.0} px (frame {a} to {b})");
        }
        if played.looping
            && played.frames.len() > 2
            && let (Some(first), Some(last)) = (
                played.frames.first().and_then(|&f| boxes[f]),
                played.frames.last().and_then(|&f| boxes[f]),
            )
        {
            let (a, b) = (middle(last), middle(first));
            let back = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let usual = steps.iter().map(|s| s.2).sum::<f32>() / steps.len().max(1) as f32;
            line += &format!(", last frame to first {back:.0} px");
            if back > (usual * 2.5).max(4.0) {
                warnings.push(format!(
                    "\"{}\" loops, but its last frame and its first are {back:.0} px apart (the \
                     steps are about {usual:.0}): it pops when it starts again. Fine for a walk \
                     across the screen; for a walk on the spot, keep the body in place",
                    played.name
                ));
            }
        }
        motion.push(line);
    }
    say!(
        "OK: {} (looked at in {:.2} s{})",
        options.path.display(),
        started.elapsed().as_secs_f32(),
        match warnings.len() {
            0 => String::new(),
            1 => ", 1 warning".into(),
            n => format!(", {n} warnings"),
        }
    );
    say!("  canvas {} x {}", document.width(), document.height());
    for line in frames_summary(&document) {
        say!("  {line}");
    }
    say!("  what's drawn on each frame (the animated layers):");
    for (frame, area) in boxes.iter().enumerate() {
        match area {
            Some(a) => {
                let (x, y) = middle(*a);
                say!(
                    "    {frame}: x {}..{}, y {}..{}, middle ({x:.0}, {y:.0})",
                    a.left,
                    a.right,
                    a.top,
                    a.bottom
                );
            }
            None => say!("    {frame}: empty"),
        }
    }
    say!("  motion:");
    for line in motion {
        say!("    {line}");
    }
    if !warnings.is_empty() {
        say!("  warnings:");
        for warning in warnings {
            say!("    {warning}");
        }
    }
    Ok(())
}

/// Frame `frame` with only its animated layers (`animated`) or only its
/// still ones showing.
fn layers_of(document: &Document, frame: usize, animated: bool, background: Background) -> Image {
    let mut picture = document.frame_picture(frame);
    let hidden: Vec<_> = picture
        .layers()
        .iter()
        .filter(|l| l.animated() != animated)
        .map(|l| l.id())
        .collect();
    for id in hidden {
        if let Some(layer) = picture.layer_mut(id) {
            layer.set_visible(false);
        }
    }
    Image::of_document(&picture, background)
}

/// A file name for one animation's file: `<stem>-<animation>`, or just
/// `<stem>` when the animation is the whole picture, named after it.
fn file_stem_for(stem: &str, animation: &str) -> String {
    if animation == stem {
        stem.to_owned()
    } else {
        format!("{stem}-{animation}")
    }
}

fn picture_of(document: &Document, frame: usize, transparent: bool) -> Image {
    let background = if transparent {
        Background::Transparent
    } else {
        Background::Paper
    };
    Image::of_document(&document.frame_picture(frame), background)
}

fn save(image: &Image, path: &Path, alpha: bool, saved: &mut Vec<String>) -> Result<(), Problem> {
    image
        .save_png(path, alpha)
        .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
    saved.push(path.display().to_string());
    Ok(())
}

fn make_out(out: &Path) -> Result<(), Problem> {
    std::fs::create_dir_all(out)
        .map_err(|e| failed(format!("can't make the folder {}: {e}", out.display())))
}

/// `quill render picture.quill`: frames as pictures to look at.
pub fn render(args: &[String]) -> Result<(), Problem> {
    let options = parse(
        args,
        &[
            "--out",
            "--frame",
            "--all-frames",
            "--frame-sheet",
            "--onion",
            "--transparent",
            "--scale",
        ],
    )?;
    let document = open(&options.path)?;
    let name = stem(&options.path);
    make_out(&options.out)?;
    let count = document.frame_count();
    let scale = options.scale;
    let enlarge = |image: Image| {
        if scale > 1 {
            image.magnify(scale)
        } else {
            image
        }
    };
    let mut saved = Vec::new();
    let shown = check_frame(&document, options.frame.unwrap_or(document.frame()))?;
    if options.all_frames {
        for frame in 0..count {
            let path = options.out.join(format!("{name}-{frame:03}.png"));
            save(
                &enlarge(picture_of(&document, frame, options.transparent)),
                &path,
                options.transparent,
                &mut saved,
            )?;
        }
    } else {
        let path = options.out.join(format!("{name}-{shown:03}.png"));
        save(
            &enlarge(picture_of(&document, shown, options.transparent)),
            &path,
            options.transparent,
            &mut saved,
        )?;
    }
    if options.frame_sheet {
        let frames: Vec<(u32, Image)> = (0..count)
            .map(|f| (f as u32, picture_of(&document, f, false)))
            .collect();
        let columns = (count as f32).sqrt().ceil() as usize;
        let title = format!("{} - {count} frames", options.path.display());
        let path = options.out.join(format!("{name}-frames.png"));
        save(
            &frame_sheet(&title, &frames, columns.max(1)),
            &path,
            false,
            &mut saved,
        )?;
    }
    if options.onion {
        // The frame with the one before (red) and after (blue) faint over
        // it: only what changes between frames (the animated layers).
        let (start, end, _, looping) = document.timeline().playing_range(shown);
        let neighbour = |offset: i64| -> Option<usize> {
            let n = shown as i64 + offset;
            if (start as i64..end as i64).contains(&n) {
                Some(n as usize)
            } else if looping && end - start > 1 {
                Some(start + (n - start as i64).rem_euclid((end - start) as i64) as usize)
            } else {
                None
            }
        };
        // The still layers, the ghosts over them, the frame's drawing on
        // top (as the app shows it). With no animated layer, all of it.
        let any_animated = document.layers().iter().any(|l| l.animated());
        let mut image = if any_animated {
            layers_of(&document, shown, false, Background::Paper)
        } else {
            Image::new(document.width(), document.height(), {
                let [r, g, b, _] = quill_engine::image::unpremultiply(document.paper());
                [r, g, b, 255]
            })
        };
        for (offset, color) in [(-1, [230, 60, 60]), (1, [60, 110, 230])] {
            if let Some(n) = neighbour(offset) {
                let ghost = layers_of(&document, n, any_animated, Background::Transparent);
                image.paste(&ghost.tinted(color, 0.35), 0, 0);
            }
        }
        image.paste(
            &layers_of(&document, shown, any_animated, Background::Transparent),
            0,
            0,
        );
        let path = options.out.join(format!("{name}-{shown:03}-onion.png"));
        save(&enlarge(image), &path, false, &mut saved)?;
    }
    say!(
        "Rendered {}: {} x {}, {count} frame{}",
        options.path.display(),
        document.width(),
        document.height(),
        if count == 1 { "" } else { "s" }
    );
    say!("  saved: {}", saved.join(", "));
    Ok(())
}

/// `quill export picture.quill`: the sprite sheet with its JSON (the
/// default), frames, GIFs, MP4s: the same files as for a script.
pub fn export(args: &[String]) -> Result<(), Problem> {
    let mut options = parse(
        args,
        &[
            "--out",
            "--sheet",
            "--gif",
            "--mp4",
            "--frames-png",
            "--scale",
            "--transparent",
        ],
    )?;
    if !(options.sheet || options.gif || options.mp4 || options.export_frames) {
        options.sheet = true;
    }
    let document = open(&options.path)?;
    let name = stem(&options.path);
    make_out(&options.out)?;
    let scale = options.scale;
    let enlarge = |image: &Image| {
        if scale > 1 {
            image.magnify(scale)
        } else {
            image.clone()
        }
    };
    let animations = document.animations(&name);
    let mut saved = Vec::new();
    if options.sheet {
        let image_name = format!("{name}.png");
        let (sheet, json) = document
            .sprite_sheet(&name, &image_name, scale)
            .map_err(failed)?;
        let png = options.out.join(&image_name);
        save(&sheet, &png, true, &mut saved)?;
        let json_path = options.out.join(format!("{name}.json"));
        std::fs::write(&json_path, json)
            .map_err(|e| failed(format!("couldn't save {}: {e}", json_path.display())))?;
        saved.push(json_path.display().to_string());
    }
    for played in &animations {
        let images = document.frame_images(&played.frames);
        if options.export_frames {
            for (n, frame) in played.frames.iter().enumerate() {
                let path = options
                    .out
                    .join(format!("{}-{n:03}.png", file_stem_for(&name, &played.name)));
                save(&enlarge(&images[frame]), &path, true, &mut saved)?;
            }
        }
        let paper = {
            let [r, g, b, _] = quill_engine::image::unpremultiply(document.paper());
            [r, g, b, 255]
        };
        let ticks = |on_paper: bool| -> Vec<Image> {
            played
                .ticks()
                .iter()
                .map(|f| {
                    let image = enlarge(&images[f]);
                    if on_paper {
                        image.on_color(paper)
                    } else {
                        image
                    }
                })
                .collect()
        };
        if options.gif {
            let path = options
                .out
                .join(format!("{}.gif", file_stem_for(&name, &played.name)));
            save_gif(&ticks(false), played.fps, played.looping, &path)
                .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
            saved.push(path.display().to_string());
        }
        if options.mp4 {
            let folder = options
                .out
                .join(format!("{}-mp4-frames", file_stem_for(&name, &played.name)));
            make_out(&folder)?;
            for (n, image) in ticks(true).iter().enumerate() {
                image
                    .save_png(&folder.join(format!("{n:03}.png")), false)
                    .map_err(|e| failed(format!("couldn't save a frame: {e}")))?;
            }
            let mp4 = options
                .out
                .join(format!("{}.mp4", file_stem_for(&name, &played.name)));
            let command = crate::export::ffmpeg_command(&folder, played.fps, &mp4);
            let done = std::process::Command::new(&command[0])
                .args(&command[1..])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if done {
                let _ = std::fs::remove_dir_all(&folder);
                saved.push(mp4.display().to_string());
            } else {
                saved.push(format!(
                    "{} (ffmpeg didn't make the video; run: {})",
                    folder.display(),
                    crate::export::shell_words(&command)
                ));
            }
        }
    }
    let list: Vec<String> = animations
        .iter()
        .map(|a| format!("{} ({} frames, {} fps)", a.name, a.frames.len(), a.fps))
        .collect();
    say!("Exported {}: {}", options.path.display(), list.join(", "));
    let [x, y] = document.pivot();
    say!(
        "  frame {} x {}, pivot ({}, {})",
        document.width() * scale,
        document.height() * scale,
        x * scale as f32,
        y * scale as f32
    );
    for line in saved {
        say!("  saved: {line}");
    }
    Ok(())
}

/// `quill critique picture.quill [--frame n | --frames a-b]`: the
/// findings that need no script. A picture has no written intent, so
/// there's no focal point or key colours to hold it to.
pub fn critique(args: &[String]) -> Result<(), Problem> {
    let options = parse(args, &["--frame", "--frames", "--detail"])?;
    let document = open(&options.path)?;
    let subject = |frame: usize| -> crate::critique::Painted {
        let picture = document.frame_picture(frame);
        crate::critique::Painted {
            image: Image::of_document(&picture, Background::Paper),
            focal: None,
            focal_part: None,
            key_colors: Vec::new(),
            parts: Vec::new(),
            layers: crate::views::layer_parts(&picture),
            notes: Vec::new(),
        }
    };
    say!(
        "critique: {} (a picture: no intent block, so no focal point or key colours to judge \
         against; the rest is the same as for a script)",
        options.path.display()
    );
    match options.frames {
        None => {
            let frame = check_frame(&document, options.frame.unwrap_or(document.frame()))?;
            if document.frame_count() > 1 {
                say!("  frame {frame}");
            }
            let critique = subject(frame).critique();
            for line in critique.lines() {
                say!("  {line}");
            }
            if options.detail {
                for line in critique.detail_lines() {
                    say!("    {line}");
                }
            }
        }
        Some((first, last)) => {
            let last = check_frame(&document, last)?;
            let mut worst: Option<(usize, usize)> = None;
            for frame in first..=last {
                let c = subject(frame).critique();
                let warnings = c.warnings();
                say!(
                    "  frame {frame}: {} value group{}; {warnings} warning{}",
                    c.values.groups.len(),
                    if c.values.groups.len() == 1 { "" } else { "s" },
                    if warnings == 1 { "" } else { "s" }
                );
                if worst.is_none_or(|(_, w)| warnings > w) {
                    worst = Some((frame, warnings));
                }
            }
            if let Some((frame, warnings)) = worst {
                say!(
                    "  worst frame: {frame} ({warnings} warnings); critique it alone with --frame {frame}"
                );
            }
        }
    }
    Ok(())
}

/// `quill apply fix.json --to picture.quill --frame n [--layer name]
/// [--out other.quill]`: paints a script onto one frame of a picture,
/// on its layer `name` (or the one selected), and saves it. On a pixel
/// canvas the script is painted alone at the picture's size and laid
/// over that frame's drawing, whole pixels only.
pub fn apply(args: &[String]) -> Result<(), Problem> {
    let mut script_path = None;
    let mut to = None;
    let mut frame = None;
    let mut layer = None;
    let mut out = None;
    let mut every_frame = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or_else(|| Problem::Usage(format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--to" => to = Some(PathBuf::from(value("--to")?)),
            "--frame" => {
                let text = value("--frame")?;
                frame = Some(text.parse::<usize>().map_err(|_| {
                    Problem::Usage(format!("--frame needs a whole number, not {text}"))
                })?);
            }
            "--layer" => layer = Some(value("--layer")?),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--every-frame" => every_frame = true,
            flag if flag.starts_with("--") => {
                return Err(Problem::Usage(format!("unknown option {flag}")));
            }
            other if script_path.is_none() => script_path = Some(PathBuf::from(other)),
            other => {
                return Err(Problem::Usage(format!(
                    "one script at a time (also got {other})"
                )));
            }
        }
    }
    let script_path = script_path.ok_or_else(|| Problem::Usage("apply needs a script".into()))?;
    let to = to.ok_or_else(|| Problem::Usage("apply needs --to picture.quill".into()))?;
    let mut document = open(&to)?;
    let shown = document.frame();
    let selected = document.active_id();
    let frame = check_frame(&document, frame.unwrap_or(shown))?;
    document.go_to_frame(frame);
    if let Some(name) = &layer {
        let id = document
            .layers()
            .iter()
            .find(|l| l.name() == name)
            .map(|l| l.id())
            .ok_or_else(|| {
                let names: Vec<&str> = document.layers().iter().map(|l| l.name()).collect();
                failed(format!(
                    "no layer called \"{name}\" (layers: {})",
                    names.join(", ")
                ))
            })?;
        document.set_active(id);
    }
    let target = document.active_layer().name().to_owned();
    // A still layer is the same on every frame: painting it changes all of
    // them, which is rarely what "fix frame 3" means.
    if document.frame_count() > 1 && !document.active_layer().animated() && !every_frame {
        let animated: Vec<&str> = document
            .layers()
            .iter()
            .filter(|l| l.animated())
            .map(|l| l.name())
            .collect();
        return Err(failed(format!(
            "layer \"{target}\" is still (the same on every frame), so this would change every \
             frame. Choose an animated layer with --layer ({}), or add --every-frame if that's \
             what you want",
            if animated.is_empty() {
                "there are none: animate one in the app".to_owned()
            } else {
                animated.join(", ")
            }
        )));
    }
    let script = load(&script_path)?;
    let problem = |e: &dyn std::fmt::Display| failed(format!("{}: {e}", script_path.display()));
    let play = PlayOptions {
        folder: script_path.parent().map(Path::to_path_buf),
        ..PlayOptions::default()
    };
    let mut player = Player::new(script, play).map_err(|e| problem(&e))?;
    if player.is_pixel() {
        // Painted alone (big, then shrunk to whole pixels), then laid over.
        let mut painted = player.create_document().map_err(|e| problem(&e))?;
        player
            .run(&mut painted, |_, _| {})
            .map_err(|e| problem(&e))?;
        if (painted.width(), painted.height()) != (document.width(), document.height()) {
            return Err(failed(format!(
                "the script's canvas is {} x {}, the picture {} x {}: make them the same",
                painted.width(),
                painted.height(),
                document.width(),
                document.height()
            )));
        }
        let image = Image::of_document(&painted, Background::Transparent);
        let surface = &mut document.active_layer_mut().surface;
        let rect = PixelRect::new(0, 0, image.width() as i32, image.height() as i32);
        let below: Vec<Rgba8> = (0..image.height() as i32)
            .flat_map(|y| (0..image.width() as i32).map(move |x| (x, y)))
            .map(|(x, y)| surface.pixel(x, y))
            .collect();
        let width = image.width() as i32;
        surface.paint_pixels(rect, |x, y| {
            let [r, g, b, a] = image.pixel(x as u32, y as u32);
            (a > 0).then(|| {
                blend_over(
                    below[(y * width + x) as usize],
                    Rgba8::from_straight(r, g, b, a),
                    1.0,
                    BlendMode::Normal,
                )
            })
        });
    } else {
        player.start_on(&mut document).map_err(|e| problem(&e))?;
        player
            .run(&mut document, |_, _| {})
            .map_err(|e| problem(&e))?;
    }
    // As it was: the frame shown and the layer selected (the owner's next
    // stroke goes where it went before).
    document.go_to_frame(shown);
    document.set_active(selected);
    let out = out.unwrap_or_else(|| to.clone());
    file::save(&document, &out)
        .map_err(|e| failed(format!("couldn't save {}: {e}", out.display())))?;
    say!(
        "Applied {} to frame {frame} of {} (layer \"{target}\")",
        script_path.display(),
        to.display()
    );
    say!("  saved: {}", out.display());
    Ok(())
}

/// `quill bake script.json [--out folder]`: a script's animations as a
/// picture with frames (`<name>.quill`), to be drawn on by hand.
pub fn bake(args: &[String]) -> Result<(), Problem> {
    let mut script_path = None;
    let mut out = PathBuf::from("out");
    let mut replace = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--replace" => replace = true,
            "--out" => {
                out = PathBuf::from(
                    rest.next()
                        .ok_or_else(|| Problem::Usage("--out needs a folder".into()))?,
                );
            }
            flag if flag.starts_with("--") => {
                return Err(Problem::Usage(format!("unknown option {flag}")));
            }
            other => script_path = Some(PathBuf::from(other)),
        }
    }
    let script_path = script_path.ok_or_else(|| Problem::Usage("bake needs a script".into()))?;
    let script = load(&script_path)?;
    let document =
        quill_engine::script::bake::bake(&script, script_path.parent().map(Path::to_path_buf))
            .map_err(|e| failed(format!("{}: {e}", script_path.display())))?;
    make_out(&out)?;
    let path = out.join(format!("{}.{}", stem(&script_path), file::FILE_EXTENSION));
    // A baked picture is for drawing on by hand: never bake over one.
    if path.exists() && !replace {
        return Err(failed(format!(
            "{} is already there (it may have been drawn on since): bake into another folder \
             with --out, or add --replace to put a fresh one in its place",
            path.display()
        )));
    }
    file::save(&document, &path)
        .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
    say!("Baked {} into {}", script_path.display(), path.display());
    for line in frames_summary(&document) {
        say!("  {line}");
    }
    say!("  (open it in Duck Quill to draw on the frames; the script is unchanged)");
    Ok(())
}
