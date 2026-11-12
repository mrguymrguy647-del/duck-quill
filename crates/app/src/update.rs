//! Updates: Duck Quill asks GitHub for its newest release, and installs
//! it with one click.
//!
//! - **What it sends:** one request to GitHub's releases page for
//!   duck-quill, saying only "Duck Quill <version>". Nothing about you,
//!   your pictures or your computer. It's the app's only use of the
//!   network. A failed check (offline, GitHub down) says nothing at start
//!   and never makes starting wait: it runs on its own thread.
//! - **Which versions:** releases only, never drafts or pre-releases, and
//!   only one newer than the version running.
//! - **The download is checked:** the setup's SHA-256 fingerprint must be
//!   the one in the `SHA256SUMS.txt` published with the same release, or
//!   it's deleted and never run.
//! - **Installing:** Duck Quill starts a copy of itself as the updater
//!   and closes. The updater waits for it to close, puts the installed
//!   files aside, runs the setup quietly, and checks the new version is
//!   there and starts. If the setup fails or anything is missing, it puts
//!   the old files back, so the version you had still works. Then it
//!   opens Duck Quill again with the same pictures (unsaved work comes
//!   back from its autosaved copy).
//! - A portable copy (from the zip) isn't installed by a setup, so it's
//!   offered a Download link instead.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Where releases are published.
pub const RELEASES_API: &str = "https://api.github.com/repos/mrguymrguy647-del/duck-quill/releases";
pub const RELEASES_PAGE: &str = "https://github.com/mrguymrguy647-del/duck-quill/releases/latest";
/// Only in builds made for the install test (`release.yml`'s
/// install-test job), never in a published one: where to ask for releases
/// (a little server on the test machine) and the version to pretend to be.
/// Set when compiling, so a published DuckQuill.exe can't be pointed
/// anywhere else (release.yml checks it doesn't contain the test address).
pub const TEST_RELEASES: Option<&str> = option_env!("DUCK_QUILL_TEST_RELEASES");
const TEST_VERSION: Option<&str> = option_env!("DUCK_QUILL_TEST_VERSION");

/// True in a build made for the install test.
pub fn test_build() -> bool {
    TEST_RELEASES.is_some()
}

/// The folder downloads go to (and the updater runs from).
pub fn download_folder() -> PathBuf {
    std::env::temp_dir().join("Duck Quill update")
}

/// In a test build: notes what happened, for the install test to read.
pub fn test_log(text: &str) {
    if test_build() {
        use std::io::Write;
        let _ = std::fs::create_dir_all(download_folder());
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(download_folder().join("test-log.txt"))
        {
            let _ = writeln!(file, "{text}");
        }
    }
}

/// In a test build: the install test asks for an update (check, then
/// update without a click) by leaving this file. Used up when seen.
pub fn test_update_asked() -> bool {
    test_build() && std::fs::remove_file(download_folder().join("update-now")).is_ok()
}

/// The file next to an installed DuckQuill.exe that says it was installed
/// by the setup (a portable copy doesn't have it).
pub const INSTALLED_MARKER: &str = "installed.ini";
/// The checksums file published with each release.
pub const CHECKSUMS: &str = "SHA256SUMS.txt";

/// A version number, like 0.6.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// "0.6.1" or "v0.6.1" (anything after a "-", like "-beta", makes it
    /// a pre-release, which is never offered).
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim().trim_start_matches('v');
        if text.contains('-') || text.contains('+') {
            return None;
        }
        let mut parts = text.split('.').map(|p| p.parse::<u32>().ok());
        let version = Version(
            parts.next()??,
            parts.next()??,
            parts.next().unwrap_or(Some(0))?,
        );
        parts.next().is_none().then_some(version)
    }

    /// The version running.
    pub fn running() -> Version {
        Version::parse(TEST_VERSION.unwrap_or(env!("CARGO_PKG_VERSION")))
            .unwrap_or(Version(0, 0, 0))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A release as GitHub's API describes it (only what's needed).
#[derive(Clone, Debug, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

/// A newer version to offer.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub version: Version,
    pub page: String,
    /// The setup and the checksums file, if the release has them.
    pub setup_url: Option<String>,
    pub checksums_url: Option<String>,
}

/// The newest release in `releases` that is newer than `running`: never
/// a draft or a pre-release, never the same version or an older one.
pub fn newest(releases: &[ReleaseInfo], running: Version) -> Option<Offer> {
    releases
        .iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| Some((Version::parse(&r.tag_name)?, r)))
        .filter(|(version, _)| *version > running)
        .max_by_key(|(version, _)| *version)
        .map(|(version, release)| {
            let setup_name = format!("DuckQuill-{version}-setup.exe");
            let url = |name: &str| {
                release
                    .assets
                    .iter()
                    .find(|a| a.name == name)
                    .map(|a| a.browser_download_url.clone())
            };
            Offer {
                version,
                page: if release.html_url.is_empty() {
                    RELEASES_PAGE.to_owned()
                } else {
                    release.html_url.clone()
                },
                setup_url: url(&setup_name),
                checksums_url: url(CHECKSUMS),
            }
        })
}

/// The SHA-256 fingerprint of `bytes`, as 64 lowercase hex digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The fingerprint `checksums` (lines of "<hex>  <file name>", as
/// `sha256sum` writes them) gives for `file_name`.
pub fn expected_sha256(checksums: &str, file_name: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file_name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Checks the setup at `setup` against `checksums`. `Err` says why it
/// must not be run.
pub fn verify(setup: &Path, checksums: &str) -> Result<(), String> {
    let name = setup
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let expected = expected_sha256(checksums, &name)
        .ok_or_else(|| format!("the release's checksums don't list {name}"))?;
    let bytes = std::fs::read(setup).map_err(|e| format!("can't read the download: {e}"))?;
    let actual = sha256_hex(&bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "the download isn't the file that was published (its fingerprint is {actual}, the \
             release says {expected})"
        ))
    }
}

/// True if the Duck Quill in `folder` was installed by the setup (not
/// unzipped by hand).
pub fn is_installed(folder: &Path) -> bool {
    folder.join(INSTALLED_MARKER).is_file()
}

/// The folder DuckQuill.exe is in.
pub fn program_folder() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

// ---- Talking to GitHub (on a thread of its own) -----------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent(format!("Duck Quill {}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Asks GitHub for the releases and picks the newest to offer.
pub fn check_now() -> Result<Option<Offer>, String> {
    let mut response = agent()
        .get(format!(
            "{}?per_page=20",
            TEST_RELEASES.unwrap_or(RELEASES_API)
        ))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let releases: Vec<ReleaseInfo> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(newest(&releases, Version::running()))
}

/// What a check finds: a newer version or none, or why it couldn't check.
pub type CheckAnswer = Result<Option<Offer>, String>;

/// Checks in the background; the answer arrives on the receiver.
pub fn check_in_background() -> Receiver<CheckAnswer> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        let _ = sender.send(check_now());
    });
    receiver
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let mut response = agent().get(url).call().map_err(|e| e.to_string())?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(500 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    std::fs::write(to, bytes).map_err(|e| format!("{}: {e}", to.display()))
}

/// Downloads the setup and the checksums into `folder` and checks them.
/// Returns the setup's path, or why it can't be used (a setup that
/// doesn't match is deleted).
pub fn download_and_verify(offer: &Offer, folder: &Path) -> Result<PathBuf, String> {
    let setup_url = offer
        .setup_url
        .as_deref()
        .ok_or("this release has no setup to download")?;
    let checksums_url = offer
        .checksums_url
        .as_deref()
        .ok_or("this release has no checksums, so its setup can't be checked")?;
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let setup = folder.join(format!("DuckQuill-{}-setup.exe", offer.version));
    let sums = folder.join(CHECKSUMS);
    download(checksums_url, &sums)?;
    download(setup_url, &setup)?;
    let checksums = std::fs::read_to_string(&sums).map_err(|e| e.to_string())?;
    if let Err(problem) = verify(&setup, &checksums) {
        let _ = std::fs::remove_file(&setup);
        return Err(problem);
    }
    // (Windows runs any .exe; elsewhere a file must be marked runnable.)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&setup, std::fs::Permissions::from_mode(0o755));
    }
    Ok(setup)
}

// ---- Installing, with a way back ---------------------------------------------------

/// What the updater does, written down beside the settings so the next
/// Duck Quill can say how it went.
pub const RESULT_FILE: &str = "update-result.txt";

/// Runs the setup and makes sure the version you had still works if it
/// doesn't: the files in `program` are copied to `backup` first; after
/// `run_setup` the new `DuckQuill.exe` must be there and `check` must
/// say it's the expected version; if either fails, the files are put back
/// exactly as they were. Returns how it went, in words.
pub fn install_with_way_back(
    program: &Path,
    backup: &Path,
    run_setup: impl FnOnce() -> Result<(), String>,
    check: impl FnOnce() -> Result<(), String>,
) -> Result<String, String> {
    let _ = std::fs::remove_dir_all(backup);
    copy_folder(program, backup).map_err(|e| format!("couldn't put the old files aside: {e}"))?;
    let outcome = run_setup().and_then(|()| check());
    match outcome {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(backup);
            Ok("updated".to_owned())
        }
        Err(problem) => {
            // Back as it was: what the setup changed or added goes, the old
            // files come back.
            let restored = restore_folder(backup, program);
            let _ = std::fs::remove_dir_all(backup);
            match restored {
                Ok(()) => Err(format!(
                    "the update didn't work ({problem}), so the version you had was put back"
                )),
                Err(e) => Err(format!(
                    "the update didn't work ({problem}), and putting the old files back failed \
                     too ({e}): please download the setup from the Releases page and run it"
                )),
            }
        }
    }
}

fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Makes `program` exactly what `backup` holds: files the setup added are
/// removed, changed ones replaced.
fn restore_folder(backup: &Path, program: &Path) -> std::io::Result<()> {
    if program.exists() {
        for entry in std::fs::read_dir(program)? {
            let entry = entry?;
            if !backup.join(entry.file_name()).exists() {
                if entry.file_type()?.is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                } else {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
    }
    copy_folder(backup, program)
}

// ---- In the app: the check, the banner, the download ---------------------------

/// What the app knows about updates.
#[derive(Default)]
pub struct Updater {
    /// A check in progress, and whether it was asked for (Help → Check
    /// for updates) rather than the quiet one at start.
    checking: Option<(Receiver<CheckAnswer>, bool)>,
    pub offer: Option<Offer>,
    downloading: Option<Receiver<Result<PathBuf, String>>>,
    /// Said in the banner: why an update couldn't go ahead.
    problem: Option<String>,
}

/// What happened, for the app to act on.
pub enum UpdateEvent {
    /// The answer to Help → Check for updates, to show in a window.
    Answer(String),
    /// The setup is downloaded and checked: time to install it.
    Ready(PathBuf),
}

impl Updater {
    pub fn check(&mut self, asked: bool) {
        if self.checking.is_none() && self.downloading.is_none() {
            self.checking = Some((check_in_background(), asked));
        }
    }

    pub fn is_busy(&self) -> bool {
        self.checking.is_some() || self.downloading.is_some()
    }

    /// Update: download the setup and check it.
    pub fn download(&mut self) {
        let Some(offer) = self.offer.clone() else {
            return;
        };
        self.problem = None;
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let _ = sender.send(download_and_verify(&offer, &download_folder()));
        });
        self.downloading = Some(receiver);
    }

    /// Picks up answers from the background.
    pub fn poll(&mut self) -> Option<UpdateEvent> {
        if let Some((receiver, asked)) = &self.checking
            && let Ok(result) = receiver.try_recv()
        {
            let asked = *asked;
            self.checking = None;
            test_log(&format!("checked: {result:?}"));
            match result {
                Ok(offer) => {
                    let answer = match &offer {
                        Some(offer) => format!("Duck Quill {} is out.", offer.version),
                        None => format!(
                            "You have the newest version (Duck Quill {}).",
                            env!("CARGO_PKG_VERSION")
                        ),
                    };
                    self.offer = offer;
                    if asked {
                        return Some(UpdateEvent::Answer(answer));
                    }
                }
                // At start a failed check says nothing.
                Err(problem) if asked => {
                    return Some(UpdateEvent::Answer(format!(
                        "Couldn't check for updates: {problem}. Are you connected to the internet?"
                    )));
                }
                Err(_) => {}
            }
        }
        if let Some(receiver) = &self.downloading
            && let Ok(result) = receiver.try_recv()
        {
            self.downloading = None;
            match result {
                Ok(setup) => {
                    test_log(&format!("downloaded and checked {}", setup.display()));
                    return Some(UpdateEvent::Ready(setup));
                }
                Err(problem) => {
                    test_log(&format!("refused: {problem}"));
                    self.problem = Some(format!("The update didn't download: {problem}."))
                }
            }
        }
        None
    }

    /// The line at the top of Home (and of the window), if any.
    pub fn banner(&self, installed: bool) -> Option<crate::home::Banner> {
        use crate::home::Banner;
        if self.downloading.is_some() {
            let version = self.offer.as_ref().map(|o| o.version.to_string());
            return Some(Banner {
                text: format!("Downloading Duck Quill {}…", version.unwrap_or_default()),
                button: None,
            });
        }
        if let Some(problem) = &self.problem {
            return Some(Banner {
                text: problem.clone(),
                button: Some("Try again"),
            });
        }
        let offer = self.offer.as_ref()?;
        Some(Banner {
            text: format!("Duck Quill {} is out:", offer.version),
            button: Some(if installed { "Update" } else { "Download" }),
        })
    }
}

/// Opens a web page in the browser.
pub fn open_in_browser(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Starts the updater (a copy of this program, run from the temporary
/// folder so the program's own files can be replaced) and returns: the
/// app then closes, and the updater waits for that.
pub fn start_updater(setup: &Path, version: Version, reopen: &[PathBuf]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let program = exe.parent().ok_or("no program folder")?;
    let folder = setup.parent().ok_or("no download folder")?;
    let helper = folder.join(if cfg!(windows) {
        "DuckQuill-updater.exe"
    } else {
        "DuckQuill-updater"
    });
    std::fs::copy(&exe, &helper).map_err(|e| format!("{}: {e}", helper.display()))?;
    let mut command = std::process::Command::new(&helper);
    command
        .arg(APPLY_UPDATE)
        .arg(setup)
        .arg("--program")
        .arg(program)
        .arg("--version")
        .arg(version.to_string());
    for picture in reopen {
        command.arg("--reopen").arg(picture);
    }
    command.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// ---- The updater (DuckQuill-updater.exe --apply-update ...) -----------------------

/// The first argument that makes DuckQuill.exe the updater.
pub const APPLY_UPDATE: &str = "--apply-update";
/// `DuckQuill.exe --version-to <file>` writes its version there and
/// stops: the updater's check that the new version starts.
pub const VERSION_TO: &str = "--version-to";

/// What the updater was asked to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub setup: PathBuf,
    pub program: PathBuf,
    pub version: Version,
    pub reopen: Vec<PathBuf>,
}

impl Job {
    /// From the command line after `--apply-update`.
    pub fn from_args(args: &[std::ffi::OsString]) -> Option<Job> {
        let mut args = args.iter();
        let setup = PathBuf::from(args.next()?);
        let (mut program, mut version, mut reopen) = (None, None, Vec::new());
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--program") => program = Some(PathBuf::from(args.next()?)),
                Some("--version") => version = Version::parse(args.next()?.to_str()?),
                Some("--reopen") => reopen.push(PathBuf::from(args.next()?)),
                _ => return None,
            }
        }
        Some(Job {
            setup,
            program: program?,
            version: version?,
            reopen,
        })
    }
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// True if nothing has `file` open to run it (Windows won't let a running
/// program's file be written; neither will Linux).
fn free(file: &Path) -> bool {
    !file.exists() || std::fs::OpenOptions::new().append(true).open(file).is_ok()
}

/// Does the update, telling `say` how it's going. Returns how it went.
pub fn apply(job: &Job, say: &dyn Fn(String)) -> Result<String, String> {
    let app = job.program.join(exe_name("DuckQuill"));
    let quill = job.program.join(exe_name("quill"));
    // Wait for Duck Quill (all its windows) and quill to close.
    let start = std::time::Instant::now();
    loop {
        let (app_free, quill_free) = (free(&app), free(&quill));
        if app_free && quill_free {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(600) {
            return Err(
                "the update didn't start: Duck Quill or quill.exe was still running after 10 \
                 minutes, so nothing was changed"
                    .into(),
            );
        }
        say(if !app_free {
            "Waiting for Duck Quill to close (close any other Duck Quill windows)…".into()
        } else {
            "Waiting for quill.exe to finish (an agent may be using it)…".into()
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    say(format!("Installing Duck Quill {}…", job.version));
    let backup = job
        .setup
        .parent()
        .unwrap_or(Path::new("."))
        .join("old version");
    let log = job.setup.with_extension("log");
    let result = install_with_way_back(
        &job.program,
        &backup,
        || {
            let status = std::process::Command::new(&job.setup)
                .args([
                    "/VERYSILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/NOCANCEL",
                    "/SP-",
                ])
                .arg(format!("/LOG={}", log.display()))
                .arg(format!("/DIR={}", job.program.display()))
                .status()
                .map_err(|e| format!("the setup didn't start: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "the setup stopped with code {}",
                    status.code().unwrap_or(-1)
                ))
            }
        },
        || check_version(&app, job.version),
    );
    let _ = std::fs::remove_file(&job.setup);
    result.map(|_| format!("Duck Quill was updated to {}.", job.version))
}

/// Starts the new DuckQuill.exe just far enough to say its version.
fn check_version(app: &Path, version: Version) -> Result<(), String> {
    let answer =
        std::env::temp_dir().join(format!("duck-quill-version-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&answer);
    let mut child = std::process::Command::new(app)
        .arg(VERSION_TO)
        .arg(&answer)
        .spawn()
        .map_err(|e| format!("the new Duck Quill didn't start: {e}"))?;
    let start = std::time::Instant::now();
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            let _ = child.kill();
            return Err("the new Duck Quill didn't answer".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let said = std::fs::read_to_string(&answer).unwrap_or_default();
    let _ = std::fs::remove_file(&answer);
    if Version::parse(&said) == Some(version) {
        Ok(())
    } else {
        Err(format!(
            "the program installed says it's version \"{}\", not {version}",
            said.trim()
        ))
    }
}

/// The updater's small window: what's happening, then it closes itself
/// and opens Duck Quill again.
pub fn run_updater(job: Job, settings_folder: Option<PathBuf>) -> eframe::Result {
    use std::sync::{Arc, Mutex};
    let status = Arc::new(Mutex::new("Starting…".to_owned()));
    let done = Arc::new(Mutex::new(None::<Result<String, String>>));
    {
        let (status, done, job) = (status.clone(), done.clone(), job.clone());
        std::thread::spawn(move || {
            let say = |text: String| *status.lock().unwrap() = text;
            let result = apply(&job, &say);
            let text = match &result {
                Ok(text) => text.clone(),
                Err(problem) => sentence(problem),
            };
            test_log(&format!("updater: {text}"));
            if let Some(folder) = &settings_folder {
                let _ = std::fs::create_dir_all(folder);
                let _ = std::fs::write(folder.join(RESULT_FILE), text);
            }
            // Open Duck Quill again, with the same pictures (whichever
            // version is there now).
            let app = job.program.join(exe_name("DuckQuill"));
            let _ = std::process::Command::new(app)
                .args(job.reopen.iter().filter(|p| p.exists()))
                .spawn();
            *done.lock().unwrap() = Some(result);
        });
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Updating Duck Quill")
            .with_inner_size([420.0, 110.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_ui_native("Updating Duck Quill", options, move |ui, _frame| {
        use eframe::egui;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(status.lock().unwrap().clone());
            });
        });
        if done.lock().unwrap().is_some() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    })
}

/// `text` as a sentence: a capital first, a full stop last.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let first = chars
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    let rest = chars.as_str().trim_end_matches('.');
    format!("{first}{rest}.")
}

/// What the last update said, once (then it's forgotten).
pub fn take_result(settings_folder: Option<&Path>) -> Option<String> {
    let file = settings_folder?.join(RESULT_FILE);
    let text = std::fs::read_to_string(&file).ok()?;
    let _ = std::fs::remove_file(&file);
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, prerelease: bool) -> ReleaseInfo {
        let version = tag.trim_start_matches('v');
        ReleaseInfo {
            tag_name: tag.into(),
            draft,
            prerelease,
            html_url: format!("https://example/{tag}"),
            assets: vec![
                Asset {
                    name: format!("DuckQuill-{version}-setup.exe"),
                    browser_download_url: format!("https://example/{tag}/setup.exe"),
                },
                Asset {
                    name: CHECKSUMS.into(),
                    browser_download_url: format!("https://example/{tag}/sums"),
                },
            ],
        }
    }

    #[test]
    fn versions_are_read_and_pre_releases_are_not_versions() {
        assert_eq!(Version::parse("v0.6.1"), Some(Version(0, 6, 1)));
        assert_eq!(Version::parse("1.2"), Some(Version(1, 2, 0)));
        assert_eq!(Version::parse("0.7.0-beta"), None);
        assert_eq!(Version::parse("nightly"), None);
        assert!(Version(0, 10, 0) > Version(0, 9, 9), "numbers, not letters");
        assert_eq!(Version::running().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn only_a_newer_real_release_is_offered() {
        let running = Version(0, 6, 0);
        let releases = vec![
            release("v0.5.0", false, false),
            release("v0.6.0", false, false),
            release("v0.9.0", true, false),      // a draft
            release("v0.8.0", false, true),      // a pre-release
            release("v0.7.0-rc1", false, false), // a pre-release by its name
            release("v0.6.2", false, false),
            release("v0.6.1", false, false),
        ];
        let offer = newest(&releases, running).unwrap();
        assert_eq!(offer.version, Version(0, 6, 2));
        assert_eq!(
            offer.setup_url.as_deref(),
            Some("https://example/v0.6.2/setup.exe")
        );
        assert_eq!(
            offer.checksums_url.as_deref(),
            Some("https://example/v0.6.2/sums")
        );
        // The same version, or older, is never offered.
        assert_eq!(newest(&releases[..2], running), None);
        assert_eq!(newest(&[release("v0.6.0", false, false)], running), None);
        // Only drafts and pre-releases newer: nothing.
        assert_eq!(newest(&releases[2..5], running), None);
    }

    #[test]
    fn a_setup_runs_only_if_its_fingerprint_is_the_published_one() {
        let folder = std::env::temp_dir().join(format!("quill-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let setup = folder.join("DuckQuill-0.6.1-setup.exe");
        std::fs::write(&setup, b"the real setup").unwrap();
        let good = format!(
            "{}  DuckQuill-0.6.1-setup.exe\n{}  quill-linux-x86_64.tar.gz\n",
            sha256_hex(b"the real setup"),
            sha256_hex(b"other")
        );
        assert_eq!(verify(&setup, &good), Ok(()));
        // One byte different: refused.
        std::fs::write(&setup, b"the real setuP").unwrap();
        let refused = verify(&setup, &good).unwrap_err();
        assert!(
            refused.contains("isn't the file that was published"),
            "{refused}"
        );
        // Not listed: refused.
        let missing = verify(&setup, "abc  other.exe\n").unwrap_err();
        assert!(missing.contains("don't list"), "{missing}");
        // A known fingerprint, so the method can't drift.
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    fn program_with(folder: &Path, files: &[(&str, &str)]) {
        let _ = std::fs::remove_dir_all(folder);
        std::fs::create_dir_all(folder.join("skills")).unwrap();
        for (name, text) in files {
            std::fs::write(folder.join(name), text).unwrap();
        }
    }

    fn read(folder: &Path, name: &str) -> Option<String> {
        std::fs::read_to_string(folder.join(name)).ok()
    }

    #[test]
    fn a_setup_that_fails_halfway_leaves_the_old_version_as_it_was() {
        let root = std::env::temp_dir().join(format!("quill-way-back-{}", std::process::id()));
        let (program, backup) = (root.join("Duck Quill"), root.join("backup"));
        program_with(
            &program,
            &[("DuckQuill.exe", "0.6.0"), ("skills/SKILL.md", "old")],
        );
        // The setup replaces one file, adds another, then fails.
        let result = install_with_way_back(
            &program,
            &backup,
            || {
                std::fs::write(program.join("DuckQuill.exe"), "0.6.1 half-written").unwrap();
                std::fs::write(program.join("new.dll"), "x").unwrap();
                Err("the setup stopped with code 5".into())
            },
            || unreachable!("not checked after a failure"),
        );
        let problem = result.unwrap_err();
        assert!(
            problem.contains("the version you had was put back"),
            "{problem}"
        );
        assert_eq!(read(&program, "DuckQuill.exe").as_deref(), Some("0.6.0"));
        assert_eq!(read(&program, "skills/SKILL.md").as_deref(), Some("old"));
        assert!(
            !program.join("new.dll").exists(),
            "what the setup added is gone"
        );
        assert!(!backup.exists(), "the copy aside is tidied away");

        // The setup says it worked, but the new version doesn't start
        // (the check fails): put back too.
        let result = install_with_way_back(
            &program,
            &backup,
            || {
                std::fs::write(program.join("DuckQuill.exe"), "broken").unwrap();
                Ok(())
            },
            || Err("DuckQuill.exe didn't start".into()),
        );
        assert!(result.is_err());
        assert_eq!(read(&program, "DuckQuill.exe").as_deref(), Some("0.6.0"));

        // A setup that works: the new files stay.
        let result = install_with_way_back(
            &program,
            &backup,
            || {
                std::fs::write(program.join("DuckQuill.exe"), "0.6.1").unwrap();
                Ok(())
            },
            || Ok(()),
        );
        assert_eq!(result.as_deref(), Ok("updated"));
        assert_eq!(read(&program, "DuckQuill.exe").as_deref(), Some("0.6.1"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_updater_reads_what_it_was_asked() {
        let args: Vec<std::ffi::OsString> = [
            "C:/Temp/DuckQuill-0.6.1-setup.exe",
            "--program",
            "C:/Programs/Duck Quill",
            "--version",
            "0.6.1",
            "--reopen",
            "C:/pond.quill",
        ]
        .iter()
        .map(Into::into)
        .collect();
        let job = Job::from_args(&args).unwrap();
        assert_eq!(job.version, Version(0, 6, 1));
        assert_eq!(job.program, PathBuf::from("C:/Programs/Duck Quill"));
        assert_eq!(job.reopen, vec![PathBuf::from("C:/pond.quill")]);
        assert!(Job::from_args(&args[..3]).is_none(), "no version");
    }

    #[test]
    fn an_installed_copy_is_told_from_a_portable_one() {
        let folder = std::env::temp_dir().join(format!("quill-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        assert!(!is_installed(&folder), "unzipped: portable");
        std::fs::write(folder.join(INSTALLED_MARKER), "[DuckQuill]\ninstalled=1\n").unwrap();
        assert!(is_installed(&folder));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
