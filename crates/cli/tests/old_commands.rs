//! Every `quill` command from 0.5.0 keeps working, and keeps saying and
//! saving what it did.
//!
//! `tests/fixtures/0.5.0/` holds what Duck Quill 0.5.0's `quill` printed
//! for a list of commands, and the sprite sheet and tileset files it
//! wrote, made with that code before phase 6 changed anything. This test
//! runs the same commands with today's `quill` and compares.
//!
//! Times ("in 0.07 s") are left out of the comparison. On Linux, where
//! the fixtures were made, everything else must match exactly. Other
//! systems round sines and cosines in the last digit differently (see
//! the engine's `tests/reference.rs`), so there the commands must
//! succeed and print the same number of lines.
//!
//! When a change is *meant* to change what a command prints, make the
//! fixtures again with `QUILL_UPDATE_FIXTURES=1 cargo test -p quill_cli
//! --test old_commands`, read the difference, and say why in the commit.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/0.5.0")
}

/// The commands, each with the name of its fixture. `<out>` is a fresh
/// folder for the files a command saves.
const COMMANDS: &[(&str, &[&str])] = &[
    ("check-sunset", &["check", "examples/sunset.json"]),
    ("check-night-hills", &["check", "examples/night-hills.json"]),
    (
        "check-two-ducks-detail",
        &["check", "examples/two-ducks.json", "--detail"],
    ),
    ("check-duck-walk", &["check", "examples/duck-walk.json"]),
    ("check-duck-sheet", &["check", "examples/duck-sheet.json"]),
    (
        "check-duck-animated",
        &["check", "examples/duck-animated.json"],
    ),
    ("check-duck-pixel", &["check", "examples/duck-pixel.json"]),
    (
        "check-duck-forge-title",
        &["check", "examples/duck-forge-title.json"],
    ),
    (
        "check-sunset-pond-simple",
        &["check", "examples/sunset-pond-simple.json"],
    ),
    (
        "check-all-commands",
        &["check", "crates/engine/tests/scripts/all-commands.json"],
    ),
    ("info-sunset", &["info", "examples/sunset.json"]),
    (
        "info-picture",
        &["info", "crates/engine/tests/fixtures/picture-0.5.0.quill"],
    ),
    (
        "info-old-picture",
        &["info", "crates/engine/tests/fixtures/old-picture.artapp"],
    ),
    (
        "render-two-ducks",
        &[
            "render",
            "examples/two-ducks.json",
            "--preview",
            "--out",
            "<out>",
        ],
    ),
    (
        "render-duck-animated",
        &[
            "render",
            "examples/duck-animated.json",
            "--animation",
            "walk",
            "--frame",
            "3",
            "--out",
            "<out>",
        ],
    ),
    (
        "render-duck-pixel-pixels",
        &[
            "render",
            "examples/duck-pixel.json",
            "--pixels",
            "--out",
            "<out>",
        ],
    ),
    (
        "render-tile",
        &[
            "render",
            "examples/duck-pixel.json",
            "--tile",
            "--out",
            "<out>",
        ],
    ),
    (
        "export-duck-animated",
        &["export", "examples/duck-animated.json", "--out", "<out>"],
    ),
    (
        "export-duck-pixel",
        &[
            "export",
            "examples/duck-pixel.json",
            "--scale",
            "2",
            "--frames",
            "--gif",
            "--sheet",
            "--out",
            "<out>",
        ],
    ),
    (
        "atlas",
        &[
            "atlas",
            "examples/duck-pixel.json",
            "examples/duck-walk.json",
            "--name",
            "set",
            "--padding",
            "1",
            "--out",
            "<out>",
        ],
    ),
    ("critique-sunset", &["critique", "examples/sunset.json"]),
    (
        "critique-two-ducks-score",
        &["critique", "examples/two-ducks.json", "--score"],
    ),
    (
        "views-two-ducks",
        &["views", "examples/two-ducks.json", "--out", "<out>"],
    ),
    (
        "palette-sunset",
        &["palette", "examples/sunset.json", "--out", "<out>"],
    ),
    (
        "values-sunset",
        &["values", "examples/sunset.json", "--out", "<out>"],
    ),
    ("measure", &["measure", "Duck Quill", "--size", "32"]),
    ("presets", &["presets"]),
    ("help", &["help"]),
    ("help-render", &["render", "--help"]),
    (
        "no-animations",
        &["export", "examples/sunset.json", "--out", "<out>"],
    ),
    ("unknown-command", &["paint-it-black"]),
];

/// Files a command saved that are kept as fixtures too: (command,
/// file in `<out>`).
const SAVED: &[(&str, &str)] = &[
    ("export-duck-animated", "duck-animated.json"),
    ("export-duck-animated", "duck-animated.png"),
    ("export-duck-pixel", "duck-pixel.json"),
    ("export-duck-pixel", "duck-pixel.png"),
    ("export-duck-pixel", "duck-pixel-walk-002.png"),
    ("atlas", "set.json"),
    ("atlas", "set.png"),
];

/// What a command printed, with times and the output folder taken out,
/// and the exit code at the end.
fn run(name: &str, args: &[&str], out: &Path) -> String {
    let out_text = out.display().to_string();
    let args: Vec<String> = args.iter().map(|a| a.replace("<out>", &out_text)).collect();
    let output = Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(&args)
        .current_dir(root())
        .output()
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut text = String::new();
    for (label, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
        text.push_str(&format!("--- {label}\n"));
        text.push_str(&String::from_utf8_lossy(bytes));
    }
    text.push_str(&format!(
        "--- exit code {}\n",
        output.status.code().unwrap_or(-1)
    ));
    normalise(&text.replace(&out_text, "<out>"))
}

/// Without the things that differ from run to run or system to system:
/// times, Windows' line endings and backslashes, the version number.
fn normalise(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\\', "/");
    let mut result = String::new();
    for line in text.lines() {
        let mut line = line.to_owned();
        // "in 0.07 s", "(painted in 1.20 s, ...": the number goes.
        while let Some(at) = line.find(" in ") {
            let rest = &line[at + 4..];
            let digits = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            if digits > 0 && rest[digits..].starts_with(" s") {
                line = format!("{} in<time>{}", &line[..at], &rest[digits + 2..]);
            } else {
                // Not a time: hide this " in " from the search and go on.
                line = format!("{} in\u{1}{}", &line[..at], rest);
            }
        }
        let line = line
            .replace(" in\u{1}", " in ")
            .replace(env!("CARGO_PKG_VERSION"), "<version>");
        result.push_str(line.trim_end());
        result.push('\n');
    }
    result
}

fn same_png(a: &Path, b: &Path) -> bool {
    use quill_engine::image::Image;
    let (a, b) = (Image::load(a).unwrap(), Image::load(b).unwrap());
    a.width() == b.width()
        && a.height() == b.height()
        && (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)))
}

#[test]
fn every_0_5_0_command_still_prints_and_saves_the_same() {
    let update = std::env::var_os("QUILL_UPDATE_FIXTURES").is_some();
    let exact = cfg!(target_os = "linux");
    let mut problems = Vec::new();
    for (name, args) in COMMANDS {
        let out = std::env::temp_dir().join(format!("quill-old-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let printed = run(name, args, &out);
        let fixture = fixtures().join(format!("{name}.txt"));
        if update {
            std::fs::write(&fixture, &printed).unwrap();
        }
        let expected = normalise(&std::fs::read_to_string(&fixture).unwrap_or_default());
        if exact {
            if printed != expected {
                problems.push(format!(
                    "{name}: quill {} printed\n{printed}\nbut 0.5.0 printed\n{expected}",
                    args.join(" ")
                ));
            }
        } else if printed.lines().count() != expected.lines().count()
            || printed.lines().last() != expected.lines().last()
        {
            problems.push(format!("{name}: a different number of lines or exit code"));
        }
        for (_, file) in SAVED.iter().filter(|(command, _)| command == name) {
            let (made, kept) = (out.join(file), fixtures().join(file));
            if update {
                std::fs::copy(&made, &kept).unwrap();
            }
            let same = if file.ends_with(".png") {
                made.exists() && (!exact || same_png(&made, &kept))
            } else {
                let read = |p: &Path| {
                    std::fs::read_to_string(p)
                        .unwrap_or_default()
                        .replace("\r\n", "\n")
                };
                read(&made) == read(&kept)
            };
            if !same {
                problems.push(format!("{name}: {file} isn't what 0.5.0 saved"));
            }
        }
        let _ = std::fs::remove_dir_all(&out);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n\n"));
}
