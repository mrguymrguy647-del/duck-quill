//! What Duck Quill is told when it's started: the picture to open, and
//! the options another program (Duck Mold) can add.
//!
//! ```text
//! DuckQuill.exe [picture] [--paper #RRGGBB] [--guide guide.png]
//! ```
//!
//! - The first word that isn't an option is the picture to open: a
//!   `.png`, a `.quill` (or old `.artapp`), or a `.json` painting script.
//! - `--paper #RRGGBB` (or `#RGB`): the paper colour shown behind the picture.
//! - `--guide guide.png`: a picture drawn faintly over the canvas, to
//!   paint by.
//!
//! Options may come before or after the picture, and `--paper=#RRGGBB`
//! works as well as `--paper #RRGGBB`. Anything starting with `-` is an
//! option, so a picture whose name starts with `-` must be given with its
//! folder (`.\-odd.png`). Nothing here can stop Duck Quill from starting:
//! an option it doesn't know, or one with its value missing or wrong, is
//! left out and listed in `problems`, which the app shows in a message.

use std::ffi::OsString;
use std::path::PathBuf;

use quill_engine::color::from_hex;

/// What the command line asked for.
#[derive(Debug, Default, PartialEq)]
pub struct StartOptions {
    /// The picture or painting script to open.
    pub file: Option<PathBuf>,
    /// `--paper`: the paper colour (red, green, blue).
    pub paper: Option<[u8; 3]>,
    /// `--guide`: the picture to draw over the canvas.
    pub guide: Option<PathBuf>,
    /// What was left out and why, in plain words, one sentence each.
    pub problems: Vec<String>,
}

impl StartOptions {
    /// What this run of Duck Quill was started with.
    pub fn from_command_line() -> StartOptions {
        // (The first word is the program itself.)
        parse(std::env::args_os().skip(1))
    }

    /// The problems as one message, or `None` if there were none.
    pub fn problems_text(&self) -> Option<String> {
        (!self.problems.is_empty()).then(|| self.problems.join("\n\n"))
    }
}

/// Reads the words after the program's name.
pub fn parse(words: impl IntoIterator<Item = OsString>) -> StartOptions {
    let mut options = StartOptions::default();
    let mut words = words.into_iter().peekable();
    while let Some(word) = words.next() {
        if !is_option(&word) {
            match &options.file {
                None => options.file = Some(PathBuf::from(word)),
                Some(_) => options.problems.push(format!(
                    "Duck Quill opens one picture at a time, so \"{}\" was left out.",
                    word.to_string_lossy()
                )),
            }
            continue;
        }
        let text = word.to_string_lossy().into_owned();
        // "--paper=#FF8800" carries its value; "--paper #FF8800" has it
        // in the next word (unless that's another option).
        let (name, mut value) = match text.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(OsString::from(value))),
            None => (text, None),
        };
        if !matches!(name.as_str(), "--paper" | "--guide") {
            options.problems.push(format!(
                "Duck Quill doesn't know the option \"{name}\", so it was left out."
            ));
            continue;
        }
        if value.is_none() {
            value = words.next_if(|next| !is_option(next));
        }
        let value = value.filter(|value| !value.is_empty());
        match (name.as_str(), value) {
            ("--paper", Some(value)) => {
                let value = value.to_string_lossy();
                match from_hex(&value) {
                    Some(color) => options.paper = Some(color),
                    None => options.problems.push(format!(
                        "\"--paper\" needs a colour like #FF8800, and \"{value}\" isn't one, \
                         so the paper keeps its usual colour."
                    )),
                }
            }
            ("--paper", None) => options.problems.push(
                "\"--paper\" needs a colour after it, like --paper #FF8800, \
                 so the paper keeps its usual colour."
                    .to_owned(),
            ),
            (_, Some(value)) => options.guide = Some(PathBuf::from(value)),
            (_, None) => options.problems.push(
                "\"--guide\" needs a picture file after it, like --guide guide.png, \
                 so no guide is shown."
                    .to_owned(),
            ),
        }
    }
    options
}

/// True if `word` is an option: it starts with `-`.
fn is_option(word: &OsString) -> bool {
    word.to_string_lossy().starts_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(words: &[&str]) -> StartOptions {
        parse(words.iter().map(OsString::from))
    }

    #[test]
    fn nothing_or_just_a_file() {
        assert_eq!(read(&[]), StartOptions::default());
        let options = read(&["robot_face.png"]);
        assert_eq!(options.file, Some(PathBuf::from("robot_face.png")));
        assert_eq!(options.paper, None);
        assert_eq!(options.guide, None);
        assert!(options.problems.is_empty());
        assert_eq!(options.problems_text(), None);
    }

    #[test]
    fn options_may_come_before_or_after_the_file() {
        let expected = StartOptions {
            file: Some(PathBuf::from("face.png")),
            paper: Some([0x33, 0x66, 0x99]),
            guide: Some(PathBuf::from("guide.png")),
            problems: Vec::new(),
        };
        let orders: [&[&str]; 5] = [
            &["face.png", "--paper", "#336699", "--guide", "guide.png"],
            &["--paper", "#336699", "--guide", "guide.png", "face.png"],
            &["--guide", "guide.png", "face.png", "--paper", "#336699"],
            &["--paper=#336699", "face.png", "--guide=guide.png"],
            // The short colour form, and no "#".
            &["--paper", "#369", "face.png", "--guide", "guide.png"],
        ];
        for words in orders {
            assert_eq!(read(words), expected, "{words:?}");
        }
        assert_eq!(read(&["--paper", "336699"]).paper, Some([0x33, 0x66, 0x99]));
        // Given twice, the last one counts.
        assert_eq!(
            read(&["--paper", "#000", "--paper", "#FFF"]).paper,
            Some([255, 255, 255])
        );
    }

    #[test]
    fn a_missing_value_is_a_message_not_a_crash() {
        // At the end of the line.
        let options = read(&["face.png", "--paper"]);
        assert_eq!(options.file, Some(PathBuf::from("face.png")));
        assert_eq!(options.paper, None);
        assert_eq!(
            options.problems,
            [
                "\"--paper\" needs a colour after it, like --paper #FF8800, \
              so the paper keeps its usual colour."
            ]
        );
        // Followed by another option: that one is still read.
        let options = read(&["--guide", "--paper", "#FF0000", "face.png"]);
        assert_eq!(options.guide, None);
        assert_eq!(options.paper, Some([255, 0, 0]));
        assert_eq!(options.file, Some(PathBuf::from("face.png")));
        assert_eq!(
            options.problems,
            [
                "\"--guide\" needs a picture file after it, like --guide guide.png, \
              so no guide is shown."
            ]
        );
        // An empty value.
        assert_eq!(read(&["--guide="]).problems.len(), 1);
        assert_eq!(read(&["--paper", ""]).problems.len(), 1);
    }

    #[test]
    fn paper_colours() {
        let paper = |text: &str| read(&["--paper", text]);
        assert_eq!(paper("#FF8800").paper, Some([255, 136, 0]));
        assert_eq!(paper("#ff8800").paper, Some([255, 136, 0]));
        assert_eq!(paper("#F80").paper, Some([255, 136, 0]));
        assert_eq!(paper("#000000").paper, Some([0, 0, 0]));
        for bad in ["blue", "#12345", "#GGGGGG", "#FF8800FF", "#"] {
            let options = paper(bad);
            assert_eq!(options.paper, None, "{bad}");
            assert_eq!(
                options.problems,
                [format!(
                    "\"--paper\" needs a colour like #FF8800, and \"{bad}\" isn't one, \
                     so the paper keeps its usual colour."
                )]
            );
        }
    }

    #[test]
    fn an_unknown_option_is_left_out_with_a_message() {
        let options = read(&["--fullscreen", "face.png", "-x", "--paper", "#FFF"]);
        assert_eq!(options.file, Some(PathBuf::from("face.png")));
        assert_eq!(options.paper, Some([255, 255, 255]));
        assert_eq!(
            options.problems,
            [
                "Duck Quill doesn't know the option \"--fullscreen\", so it was left out.",
                "Duck Quill doesn't know the option \"-x\", so it was left out.",
            ]
        );
        let text = options.problems_text().unwrap();
        assert!(text.contains("--fullscreen") && text.contains("\"-x\""));
        // With a value stuck on, it's still unknown (and names itself).
        assert_eq!(
            read(&["--zoom=4"]).problems,
            ["Duck Quill doesn't know the option \"--zoom\", so it was left out."]
        );
    }

    #[test]
    fn a_path_that_starts_with_a_dash_is_still_an_option() {
        // Not the file: an option nobody knows.
        let options = read(&["-odd.png"]);
        assert_eq!(options.file, None);
        assert_eq!(options.problems.len(), 1);
        assert!(options.problems[0].contains("-odd.png"));
        // Nor a value: "--guide" is left without one.
        let options = read(&["--guide", "-odd.png", "face.png"]);
        assert_eq!(options.guide, None);
        assert_eq!(options.file, Some(PathBuf::from("face.png")));
        assert_eq!(options.problems.len(), 2);
        // With its folder in front, it's a file like any other.
        let dashed = PathBuf::from(".").join("-odd.png");
        let options = parse([dashed.clone().into_os_string()]);
        assert_eq!(options.file, Some(dashed));
        assert!(options.problems.is_empty());
        // Stuck on with "=", a value may start with anything.
        assert_eq!(
            read(&["--guide=-odd.png"]).guide,
            Some(PathBuf::from("-odd.png"))
        );
    }

    #[test]
    fn only_the_first_file_is_opened() {
        let options = read(&["a.png", "b.png"]);
        assert_eq!(options.file, Some(PathBuf::from("a.png")));
        assert_eq!(
            options.problems,
            ["Duck Quill opens one picture at a time, so \"b.png\" was left out."]
        );
    }
}
