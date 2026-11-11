//! `quill export`: an animated script as a sprite sheet with its JSON,
//! numbered frames, a GIF and an MP4 (see `docs/sprite-sheet-format.md`).
//! A script with no animations (or `--stills`) exports its symbols as
//! still tiles: one PNG and a JSON in the tileset format.
//!
//! Every frame is painted once, at full size with a see-through
//! background, then put into whichever files were asked for. `--scale`
//! enlarges everything with sharp pixels (pixel art stays crisp).

use std::path::{Path, PathBuf};
use std::process::Command;

use quill_engine::document::Background;
use quill_engine::gif::save_gif;
use quill_engine::image::{Color, Image, unpremultiply};
use quill_engine::script::{PlayOptions, Player, Report, Script, Sequence, parse_hex};
use quill_engine::sprites::{SheetAnimation, sprite_sheet};

use crate::atlas::{Tile, save_tiles};
use crate::regions::save_regions;
use crate::{Problem, failed, load};

/// Enlarging more than this makes files nobody wants.
const MAX_SCALE: u32 = 32;

pub struct ExportOptions {
    script: PathBuf,
    out: PathBuf,
    /// Only this animation (the sheet too).
    animation: Option<String>,
    sheet: bool,
    frames: bool,
    gif: bool,
    mp4: bool,
    scale: u32,
    /// Behind see-through parts of GIFs and MP4s.
    bg: Option<Color>,
    /// `--stills`: the script's symbols as still pictures, one tile
    /// each (what a script without animations exports anyway).
    stills: bool,
    /// Stills per row (default: about a square).
    columns: Option<u32>,
    /// See-through pixels between stills (and round the edge).
    padding: u32,
}

pub fn parse_export(args: &[String]) -> Result<ExportOptions, Problem> {
    let mut options = ExportOptions {
        script: PathBuf::new(),
        out: PathBuf::from("out"),
        animation: None,
        sheet: false,
        frames: false,
        gif: false,
        mp4: false,
        scale: 1,
        bg: None,
        stills: false,
        columns: None,
        padding: 0,
    };
    let mut script = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or_else(|| Problem::Usage(format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--out" => options.out = PathBuf::from(value("--out")?),
            "--animation" => options.animation = Some(value("--animation")?),
            "--sheet" => options.sheet = true,
            "--frames" => options.frames = true,
            "--gif" => options.gif = true,
            "--mp4" => options.mp4 = true,
            "--stills" => options.stills = true,
            "--columns" | "--padding" => {
                let (low, high) = if arg == "--columns" {
                    (1, 256)
                } else {
                    (0, 64)
                };
                let text = value(arg)?;
                let number = text
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (low..=high).contains(n))
                    .ok_or_else(|| {
                        Problem::Usage(format!(
                            "{arg} must be a whole number from {low} to {high}, not {text}"
                        ))
                    })?;
                if arg == "--columns" {
                    options.columns = Some(number);
                } else {
                    options.padding = number;
                }
            }
            "--scale" => {
                let text = value("--scale")?;
                options.scale = text
                    .parse::<u32>()
                    .ok()
                    .filter(|s| (1..=MAX_SCALE).contains(s))
                    .ok_or_else(|| {
                        Problem::Usage(format!(
                            "--scale is a whole number from 1 to {MAX_SCALE} (times bigger, \
                             sharp pixels), not {text}"
                        ))
                    })?;
            }
            "--bg" => {
                let text = value("--bg")?;
                options.bg = Some(
                    parse_hex(&text)
                        .ok()
                        .filter(|color| color[3] == 255)
                        .ok_or_else(|| {
                            Problem::Usage(format!(
                                "--bg is a colour like \"#4080A0\" (not see-through), not {text}"
                            ))
                        })?,
                );
            }
            flag if flag.starts_with("--") => {
                return Err(Problem::Usage(format!("unknown option {flag}")));
            }
            file if script.is_none() => script = Some(PathBuf::from(file)),
            extra => {
                return Err(Problem::Usage(format!(
                    "only one script at a time (also got {extra})"
                )));
            }
        }
    }
    options.script = script.ok_or_else(|| Problem::Usage("export needs a script file".into()))?;
    if !(options.sheet || options.frames || options.gif || options.mp4) {
        options.sheet = true; // the usual thing wanted
    }
    Ok(options)
}

/// `export` for stills: each symbol of the script (those in its
/// `stills` list, or all of them) painted alone on the canvas, packed
/// into `<name>.png` with `<name>.json` in the tileset format, the
/// same shape `quill atlas` writes. `--frames` also saves each one as
/// `<name>-<still>.png`.
fn export_stills(options: &ExportOptions, script: &Script, stem: &str) -> Result<(), Problem> {
    let problem = |e: &dyn std::fmt::Display| failed(format!("{}: {e}", options.script.display()));
    if options.gif || options.mp4 || options.animation.is_some() {
        return Err(failed(
            "stills don't move: --gif, --mp4 and --animation are for a script's \"animations\" \
             (stills take --sheet, --frames, --scale, --columns and --padding)",
        ));
    }
    let stills = script.still_list();
    if stills.is_empty() {
        return Err(failed(
            "there's nothing to export: this script has no animations and no symbols. `export` \
             saves a script's \"animations\" as a sprite sheet, or each `define`d symbol as a \
             still tile (use `render` for a single picture)",
        ));
    }
    let folder = options.script.parent().map(Path::to_path_buf);
    let mut tiles = Vec::with_capacity(stills.len());
    let mut canvas = (0, 0);
    // The script's regions (the same for every still: declared by the
    // script's own commands), from the first one painted.
    let mut regions: Option<Report> = None;
    for still in &stills {
        let play = PlayOptions {
            folder: folder.clone(),
            ..PlayOptions::default()
        };
        let named = |e: &dyn std::fmt::Display| problem(&format!("still \"{}\": {e}", still.name));
        let mut player = Player::new(script.still_script(still), play).map_err(|e| named(&e))?;
        let mut document = player.create_document().map_err(|e| named(&e))?;
        player
            .run(&mut document, |_, _| {})
            .map_err(|e| named(&e))?;
        canvas = (document.width(), document.height());
        regions.get_or_insert_with(|| player.report());
        let mut image = Image::of_document(&document, Background::Transparent);
        if options.scale > 1 {
            image = image.magnify(options.scale);
        }
        tiles.push(Tile {
            name: still.name.clone(),
            image,
        });
    }
    let mut saved: Vec<String> = Vec::new();
    if options.sheet {
        let sheet = save_tiles(
            &tiles,
            &options.out,
            stem,
            options.columns,
            options.padding,
            options.scale,
        )?;
        saved.push(format!(
            "{} and {} (sheet {} x {}, {} still{})",
            sheet.png.display(),
            sheet.json.display(),
            sheet.size.0,
            sheet.size.1,
            tiles.len(),
            if tiles.len() == 1 { "" } else { "s" }
        ));
        if let Some(report) = &regions
            && let Some(path) = save_regions(
                report,
                &options.out,
                stem,
                &format!("{stem}.png"),
                sheet.size,
                options.scale,
                Some((canvas.0 * options.scale, canvas.1 * options.scale)),
            )?
        {
            saved.push(format!("{} (regions, inside each still)", path.display()));
        }
    }
    if options.frames {
        std::fs::create_dir_all(&options.out).map_err(|e| {
            failed(format!(
                "can't make the folder {}: {e}",
                options.out.display()
            ))
        })?;
        for tile in &tiles {
            let path = options.out.join(format!("{stem}-{}.png", tile.name));
            tile.image
                .save_png(&path, true)
                .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
        }
        saved.push(format!("{} stills as {stem}-<name>.png", tiles.len()));
    }
    say!(
        "Exported {}: {} still{} ({})",
        options.script.display(),
        tiles.len(),
        if tiles.len() == 1 { "" } else { "s" },
        if script.stills.is_some() {
            "the script's \"stills\" list"
        } else {
            "every symbol; a \"stills\" list chooses some"
        }
    );
    say!(
        "  each {} x {}{}: {}",
        canvas.0 * options.scale,
        canvas.1 * options.scale,
        if options.scale > 1 {
            format!(" ({} times bigger, sharp pixels)", options.scale)
        } else {
            String::new()
        },
        tiles
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    for line in &saved {
        say!("  {line}");
    }
    Ok(())
}

/// One animation, painted: its frames at full size, see-through.
struct Painted {
    name: String,
    frames: Vec<Image>,
    fps: f32,
    looping: bool,
}

pub fn export(options: &ExportOptions) -> Result<(), Problem> {
    let script = load(&options.script)?;
    let problem = |e: &dyn std::fmt::Display| failed(format!("{}: {e}", options.script.display()));
    let stem = options.script.file_stem().map_or_else(
        || "animation".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    // Nothing moves: the script's symbols as still tiles.
    if script.animations.is_empty() || options.stills {
        return export_stills(options, &script, &stem);
    }
    if let Some(name) = &options.animation
        && !script.animations.contains_key(name)
    {
        let names: Vec<&str> = script.animations.keys().map(String::as_str).collect();
        return Err(failed(format!(
            "there's no animation called \"{name}\" (animations: {})",
            names.join(", ")
        )));
    }
    let folder = options.script.parent().map(Path::to_path_buf);
    std::fs::create_dir_all(&options.out).map_err(|e| {
        failed(format!(
            "can't make the folder {}: {e}",
            options.out.display()
        ))
    })?;

    // Paint every frame of every (chosen) animation.
    let mut painted = Vec::new();
    let mut paper = [255, 255, 255, 255];
    let mut canvas = (0, 0);
    let mut notes: Vec<String> = Vec::new();
    let mut regions: Option<Report> = None;
    for (name, animation) in &script.animations {
        if options
            .animation
            .as_ref()
            .is_some_and(|wanted| wanted != name)
        {
            continue;
        }
        let mut frames = Vec::with_capacity(animation.frames as usize);
        // The frames in order: static layers are painted once.
        let mut sequence = Sequence::new(
            script.clone(),
            PlayOptions {
                folder: folder.clone(),
                animation: Some(name.clone()),
                ..PlayOptions::default()
            },
        );
        for frame in 0..animation.frames {
            let (document, report) = sequence.paint(frame, |_, _| {}).map_err(|e| problem(&e))?;
            regions.get_or_insert(report);
            if frame + 1 == animation.frames
                && animation.frames > 1
                && let Some(summary) = sequence.summary(&document)
            {
                notes.push(format!("animation \"{name}\": {summary}"));
            }
            paper = unpremultiply(document.paper());
            canvas = (document.width(), document.height());
            frames.push(Image::of_document(&document, Background::Transparent));
        }
        painted.push(Painted {
            name: name.clone(),
            frames,
            fps: animation.fps,
            looping: animation.looping,
        });
    }
    let frame_count: usize = painted.iter().map(|a| a.frames.len()).sum();
    let pivot = script
        .pivot
        .unwrap_or([canvas.0 as f32 / 2.0, canvas.1 as f32]);
    let scale = options.scale;
    let enlarged = |image: &Image| {
        if scale > 1 {
            image.magnify(scale)
        } else {
            image.clone()
        }
    };
    let on_bg = |image: &Image, default: Option<Color>| match options.bg.or(default) {
        Some(color) => image.on_color(color),
        None => image.clone(),
    };
    let mut saved: Vec<String> = Vec::new();
    let save_png = |image: &Image, path: &Path| -> Result<(), Problem> {
        image
            .save_png(path, true)
            .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))
    };

    if options.sheet {
        let image_name = format!("{stem}.png");
        let animations: Vec<SheetAnimation> = painted
            .iter()
            .map(|a| SheetAnimation {
                name: a.name.clone(),
                frames: a.frames.clone(),
                fps: a.fps,
                looping: a.looping,
            })
            .collect();
        let (sheet, json) =
            sprite_sheet(&animations, &image_name, pivot, scale).map_err(|e| problem(&e))?;
        let png = options.out.join(&image_name);
        save_png(&sheet, &png)?;
        let json_path = options.out.join(format!("{stem}.json"));
        std::fs::write(&json_path, json)
            .map_err(|e| failed(format!("couldn't save {}: {e}", json_path.display())))?;
        saved.push(format!(
            "{} and {} (sheet {} x {}, {} animation{}, {frame_count} frames)",
            png.display(),
            json_path.display(),
            sheet.width(),
            sheet.height(),
            painted.len(),
            if painted.len() == 1 { "" } else { "s" }
        ));
        if let Some(report) = &regions
            && let Some(path) = save_regions(
                report,
                &options.out,
                &stem,
                &image_name,
                (sheet.width(), sheet.height()),
                scale,
                Some((canvas.0 * scale, canvas.1 * scale)),
            )?
        {
            saved.push(format!("{} (regions, inside each frame)", path.display()));
        }
    }
    if options.frames {
        let mut count = 0;
        for animation in &painted {
            for (n, frame) in animation.frames.iter().enumerate() {
                let path = options
                    .out
                    .join(format!("{stem}-{}-{n:03}.png", animation.name));
                save_png(&enlarged(frame), &path)?;
                count += 1;
            }
        }
        saved.push(format!("{count} frames as {stem}-<animation>-<frame>.png"));
    }
    if options.gif {
        for animation in &painted {
            let frames: Vec<Image> = animation
                .frames
                .iter()
                .map(|f| enlarged(&on_bg(f, None)))
                .collect();
            let path = options.out.join(format!("{stem}-{}.gif", animation.name));
            save_gif(&frames, animation.fps, animation.looping, &path)
                .map_err(|e| failed(format!("couldn't save {}: {e}", path.display())))?;
            saved.push(path.display().to_string());
        }
    }
    if options.mp4 {
        let have_ffmpeg = Command::new("ffmpeg").arg("-version").output().is_ok();
        for animation in &painted {
            // A video has no see-through background: the paper, unless --bg.
            let frames_folder = options
                .out
                .join(format!("{stem}-{}-mp4-frames", animation.name));
            std::fs::create_dir_all(&frames_folder).map_err(|e| {
                failed(format!(
                    "can't make the folder {}: {e}",
                    frames_folder.display()
                ))
            })?;
            for (n, frame) in animation.frames.iter().enumerate() {
                let path = frames_folder.join(format!("{n:03}.png"));
                save_png(&enlarged(&on_bg(frame, Some(paper))), &path)?;
            }
            let mp4 = options.out.join(format!("{stem}-{}.mp4", animation.name));
            let command = ffmpeg_command(&frames_folder, animation.fps, &mp4);
            let done = have_ffmpeg
                && Command::new(&command[0])
                    .args(&command[1..])
                    .status()
                    .is_ok_and(|status| status.success());
            if done {
                let _ = std::fs::remove_dir_all(&frames_folder);
                saved.push(mp4.display().to_string());
            } else {
                saved.push(format!(
                    "{} (frames for the video)",
                    frames_folder.display()
                ));
                notes.push(format!(
                    "{}: run this to make the video:\n    {}",
                    if have_ffmpeg {
                        "ffmpeg didn't manage"
                    } else {
                        "ffmpeg isn't installed"
                    },
                    shell_words(&command)
                ));
            }
        }
    }

    say!(
        "Exported {}: {}",
        options.script.display(),
        painted
            .iter()
            .map(|a| format!("{} ({} frames, {} fps)", a.name, a.frames.len(), a.fps))
            .collect::<Vec<_>>()
            .join(", ")
    );
    say!(
        "  frame {} x {}{}, pivot ({}, {})",
        canvas.0 * scale,
        canvas.1 * scale,
        if scale > 1 {
            format!(" ({scale} times bigger, sharp pixels)")
        } else {
            String::new()
        },
        pivot[0] * scale as f32,
        pivot[1] * scale as f32
    );
    for line in &saved {
        say!("  {line}");
    }
    if let Some(wanted) = &options.animation {
        let skipped: Vec<&str> = script
            .animations
            .keys()
            .filter(|name| *name != wanted)
            .map(String::as_str)
            .collect();
        if !skipped.is_empty() {
            say!(
                "  skipped (not --animation {wanted}): {}",
                skipped.join(", ")
            );
        }
    }
    for note in &notes {
        say!("  {note}");
    }
    Ok(())
}

/// The ffmpeg command that turns the numbered frames into an MP4 that
/// plays everywhere (H.264, even sizes, fast start).
fn ffmpeg_command(frames_folder: &Path, fps: f32, mp4: &Path) -> Vec<String> {
    [
        "ffmpeg",
        "-y",
        "-loglevel",
        "error",
        "-framerate",
        &format!("{fps}"),
        "-i",
        &frames_folder.join("%03d.png").display().to_string(),
        "-vf",
        "pad=ceil(iw/2)*2:ceil(ih/2)*2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        &mp4.display().to_string(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// A command as one line to paste into a terminal.
fn shell_words(command: &[String]) -> String {
    command
        .iter()
        .map(|word| {
            if word.chars().any(|c| c.is_whitespace() || "()*".contains(c)) {
                format!("\"{word}\"")
            } else {
                word.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn export_options_are_read_and_the_sheet_is_the_default() {
        let options = parse_export(&args("a.json")).ok().unwrap();
        assert!(options.sheet && !options.gif && !options.frames && !options.mp4);
        let options = parse_export(&args(
            "a.json --gif --scale 8 --animation walk --bg #102030",
        ))
        .ok()
        .unwrap();
        assert!(options.gif && !options.sheet);
        assert_eq!(options.scale, 8);
        assert_eq!(options.animation.as_deref(), Some("walk"));
        assert_eq!(options.bg, Some([0x10, 0x20, 0x30, 255]));
        assert!(matches!(
            parse_export(&args("a.json --scale 0")),
            Err(Problem::Usage(_))
        ));
        assert!(matches!(
            parse_export(&args("--gif")),
            Err(Problem::Usage(_))
        ));
    }

    #[test]
    fn the_ffmpeg_command_is_one_pasteable_line() {
        let command = ffmpeg_command(
            Path::new("out/x-walk-mp4-frames"),
            12.0,
            Path::new("out/x-walk.mp4"),
        );
        let line = shell_words(&command);
        assert!(
            line.starts_with("ffmpeg -y -loglevel error -framerate 12 -i "),
            "{line}"
        );
        assert!(
            line.contains("-vf \"pad=ceil(iw/2)*2:ceil(ih/2)*2\" -c:v libx264"),
            "{line}"
        );
        assert!(line.ends_with("out/x-walk.mp4"), "{line}");
    }
}
