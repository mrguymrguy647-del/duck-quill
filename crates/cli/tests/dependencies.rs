//! `quill` must stay quick to build: an agent that builds it for the
//! first time waits for every library it depends on. It depends on the
//! painting engine and a JSON library, about 45 packages in all; the
//! window app depends on about 300. (An agent once waited 8 minutes for
//! what should take under one: phase 6, fix 1.)
//!
//! This fails if `quill_cli` ever comes to depend on a window, graphics
//! or clipboard library, on the app itself, or on far more packages
//! than today.

use std::process::Command;

/// Libraries only a program with a window needs.
const WINDOW_LIBRARIES: &[&str] = &[
    "quill_app",
    "eframe",
    "egui",
    "epaint",
    "emath",
    "ecolor",
    "winit",
    "wgpu",
    "glow",
    "glutin",
    "rfd",
    "arboard",
    "accesskit",
    "windows",
    "ureq",
];

/// Today it's about 45. Room to grow, but not to double.
const MOST_PACKAGES: usize = 70;

/// The packages `package` depends on (itself included), each once.
fn packages_of(package: &str) -> Vec<String> {
    // The same cargo that's running this test.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args([
            "tree",
            "-p",
            package,
            "-e",
            "normal,build",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    // Lines are "name v1.2.3 ..." (a "(*)" marks one already listed).
    let mut packages: Vec<String> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    packages.sort_unstable();
    packages.dedup();
    packages
}

/// Libraries every program uses on Windows (files, time, threads),
/// whose names only look like window libraries.
const SYSTEM_LIBRARIES: &[&str] = &["windows-sys", "windows-link", "windows-targets"];

/// Those of `packages` that are window libraries (or parts of one,
/// like `egui-winit` or `wgpu_core`).
fn window_libraries(packages: &[String]) -> Vec<&String> {
    packages
        .iter()
        .filter(|name| !SYSTEM_LIBRARIES.contains(&name.as_str()))
        .filter(|name| !name.starts_with("windows_")) // windows-targets' parts
        .filter(|name| {
            WINDOW_LIBRARIES.iter().any(|library| {
                *name == library
                    || name.starts_with(&format!("{library}-"))
                    || name.starts_with(&format!("{library}_"))
            })
        })
        .collect()
}

#[test]
fn system_libraries_are_not_taken_for_window_libraries() {
    let names: Vec<String> = [
        "windows-sys",
        "windows-link",
        "windows_x86_64_msvc",
        "windows",
        "egui-winit",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let found: Vec<&str> = window_libraries(&names)
        .iter()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(found, ["windows", "egui-winit"]);
}

#[test]
fn quill_depends_on_no_window_library_and_stays_small() {
    let packages = packages_of("quill_cli");
    assert!(
        packages.iter().any(|name| name == "quill_engine"),
        "this doesn't look like quill's dependencies: {packages:?}"
    );
    let unwanted = window_libraries(&packages);
    assert!(
        unwanted.is_empty(),
        "quill now depends on {unwanted:?}: agents would wait for window libraries to build. \
         Keep them in the app (crates/app)."
    );
    assert!(
        packages.len() <= MOST_PACKAGES,
        "quill depends on {} packages (it was about 45): the first build gets slower with each",
        packages.len()
    );
}

/// The check above can fail: the app, which does have a window, is
/// caught by it.
#[test]
fn the_same_check_catches_the_app() {
    let packages = packages_of("quill_app");
    let found = window_libraries(&packages);
    assert!(found.iter().any(|name| *name == "eframe"), "{found:?}");
    assert!(packages.len() > MOST_PACKAGES, "{}", packages.len());
}
