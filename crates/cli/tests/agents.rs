//! AGENTS.md and CLAUDE.md, at the top of the repository, tell agents
//! what to run. Everything they name must be there: every `quill`
//! command and option, every crate and test that `-p` and `--test` name,
//! every file and folder.

use std::path::{Path, PathBuf};
use std::process::Command;

fn top() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A text file with Unix line endings, however Git checked it out.
fn read(name: &str) -> String {
    std::fs::read_to_string(top().join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .replace("\r\n", "\n")
}

fn both() -> String {
    read("AGENTS.md") + "\n" + &read("CLAUDE.md")
}

/// The code in a text: each line of a code block (without its `# comment`,
/// which is words for people), and each `inline` span.
fn code(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        if line.starts_with("```") {
            in_block = !in_block;
        } else if in_block {
            let command = line.split(" # ").next().unwrap_or(line);
            found.push(command.to_owned());
        } else {
            found.extend(line.split('`').skip(1).step_by(2).map(str::to_owned));
        }
    }
    found
}

/// Every word after `quill ` in the code: the commands it names.
fn commands(code: &[String]) -> Vec<String> {
    let mut found: Vec<String> = code
        .iter()
        .flat_map(|piece| piece.match_indices("quill ").map(move |(i, _)| &piece[i..]))
        .map(|after| {
            after["quill ".len()..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect::<String>()
        })
        .filter(|word| word.starts_with(|c: char| c.is_ascii_lowercase()))
        .collect();
    found.sort();
    found.dedup();
    found
}

fn quill(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn every_command_and_option_agents_md_names_is_there() {
    let code = code(&both());
    let commands = commands(&code);
    for command in [
        "help", "render", "check", "views", "critique", "info", "presets", "export",
    ] {
        assert!(
            commands.contains(&command.to_owned()),
            "AGENTS.md names quill {command}"
        );
    }
    for command in &commands {
        let output = if command == "help" {
            quill(&["help"])
        } else {
            quill(&[command.as_str(), "--help"])
        };
        assert!(
            output.status.success(),
            "AGENTS.md names `quill {command}`, which isn't a command"
        );
    }
    // The options on lines that run quill (cargo's own lines aside).
    let help = String::from_utf8_lossy(&quill(&["help"]).stdout).into_owned();
    let mut options = 0;
    for piece in code
        .iter()
        .filter(|piece| piece.contains("quill ") && !piece.contains("cargo "))
    {
        for word in piece.split_whitespace() {
            if let Some(option) = word.strip_prefix("--") {
                let option: String = option
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect();
                assert!(
                    help.contains(&format!("--{option}")),
                    "AGENTS.md names quill's --{option}, which it hasn't"
                );
                options += 1;
            }
        }
    }
    assert!(options >= 3, "only {options} options checked");
}

/// The crates of the workspace: (package name, its folder).
fn crates() -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(top().join("crates")).unwrap() {
        let folder = entry.unwrap().path();
        let Ok(manifest) = std::fs::read_to_string(folder.join("Cargo.toml")) else {
            continue;
        };
        // The first `name` is the package's: [package] comes first.
        let name = manifest
            .lines()
            .find_map(|line| line.trim().strip_prefix("name = \""))
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("{}: no name", folder.display()));
        found.push((name.to_owned(), folder));
    }
    found
}

#[test]
fn every_crate_test_and_file_agents_md_names_is_there() {
    let crates = crates();
    let mut crates_named = 0;
    let mut tests_named = 0;
    let mut paths_named = 0;
    for line in both().lines() {
        let words: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || "`(),".contains(c))
            .filter(|word| !word.is_empty())
            .collect();
        // The crate this line's `-p` names, if any.
        let mut folder: Option<&Path> = None;
        for pair in words.windows(2) {
            if pair[0] == "-p" {
                let (_, found) = crates
                    .iter()
                    .find(|(name, _)| name == pair[1])
                    .unwrap_or_else(|| panic!("AGENTS.md names -p {}: no such crate", pair[1]));
                folder = Some(found);
                crates_named += 1;
            }
        }
        for pair in words.windows(2) {
            if pair[0] == "--test" {
                let file = format!("{}.rs", pair[1]);
                let found = match folder {
                    Some(folder) => folder.join("tests").join(&file).is_file(),
                    None => crates
                        .iter()
                        .any(|(_, folder)| folder.join("tests").join(&file).is_file()),
                };
                assert!(found, "AGENTS.md names --test {}: no such test", pair[1]);
                tests_named += 1;
            }
        }
        for word in &words {
            let word = word
                .trim_start_matches('@')
                .trim_end_matches(['.', ':', ';', '"']);
            let looks_like_a_path = ["crates/", "docs/", "skills/", "examples/", ".github/"]
                .iter()
                .any(|start| word.starts_with(start))
                || [".md", ".toml", ".bat"]
                    .iter()
                    .any(|end| word.ends_with(end));
            if looks_like_a_path && !word.contains('<') {
                assert!(
                    top().join(word).exists(),
                    "AGENTS.md names {word}, which isn't there"
                );
                paths_named += 1;
            }
        }
    }
    assert!(crates_named >= 3, "only {crates_named} crates checked");
    assert!(tests_named >= 2, "only {tests_named} tests checked");
    assert!(paths_named >= 8, "only {paths_named} paths checked");
    assert!(
        read("CLAUDE.md").starts_with("@AGENTS.md\n"),
        "CLAUDE.md starts by bringing in AGENTS.md"
    );
}

#[test]
fn the_rust_version_is_the_one_ci_uses() {
    let ci = read(".github/workflows/ci.yml");
    let pinned = ci
        .lines()
        .find_map(|line| line.trim().strip_prefix("RUST_VERSION: \""))
        .and_then(|rest| rest.split('"').next())
        .expect("ci.yml pins RUST_VERSION");
    // "1.97.0" is said as "Rust 1.97".
    let short = pinned.strip_suffix(".0").unwrap_or(pinned);
    assert!(
        read("AGENTS.md").contains(&format!("Rust {short},")),
        "AGENTS.md should say Rust {short}, the version CI uses"
    );
}
