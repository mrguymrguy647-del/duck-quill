//! One Duck Quill at a time: double-clicking a picture while Duck Quill
//! is running opens it in that window (which comes to the front) instead
//! of starting another copy. File → New window starts a second window on
//! purpose.
//!
//! The first Duck Quill holds a lock on `instance.lock` in the settings
//! folder. A later one that finds the lock taken writes the pictures it
//! was asked to open into a small file in `open-requests`, and stops. The
//! running one looks in that folder twice a second.

use std::fs::{File, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LOCK: &str = "instance.lock";
const REQUESTS: &str = "open-requests";
/// How often the running Duck Quill looks for requests.
pub const LOOK_EVERY: Duration = Duration::from_millis(500);

/// How this Duck Quill starts.
pub enum Start {
    /// The one that answers double-clicks (or the only one, or a window
    /// opened with File → New window, which has `lock: None`).
    Running(Instance),
    /// Another Duck Quill is running and was asked to open the pictures.
    HandedOver,
}

pub struct Instance {
    /// Held while running (`None` for a second window, or with no
    /// settings folder).
    lock: Option<File>,
    requests: Option<PathBuf>,
    last_look: Instant,
}

impl Instance {
    /// A Duck Quill that doesn't take part (no settings folder, tests).
    pub fn alone() -> Instance {
        Instance {
            lock: None,
            requests: None,
            last_look: Instant::now(),
        }
    }

    /// The first Duck Quill takes the lock. A later one hands `pictures`
    /// to it, unless it's a `new_window`.
    pub fn start(folder: Option<&Path>, pictures: &[PathBuf], new_window: bool) -> Start {
        let Some(folder) = folder else {
            return Start::Running(Instance::alone());
        };
        if std::fs::create_dir_all(folder).is_err() {
            return Start::Running(Instance::alone());
        }
        let requests = folder.join(REQUESTS);
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(folder.join(LOCK))
            .ok();
        let Some(lock) = lock else {
            return Start::Running(Instance::alone());
        };
        match lock.try_lock() {
            Ok(()) => {
                // Requests left long ago (nobody was there to answer) are
                // dropped; one left a moment ago, while the last Duck
                // Quill was closing, is answered by this one.
                forget_old_requests(&requests);
                Start::Running(Instance {
                    lock: Some(lock),
                    requests: Some(requests),
                    last_look: Instant::now(),
                })
            }
            // Locks don't work here (some network drives): don't hand over.
            Err(TryLockError::Error(_)) => Start::Running(Instance::alone()),
            Err(TryLockError::WouldBlock) if new_window => Start::Running(Instance::alone()),
            Err(TryLockError::WouldBlock) => match hand_over(&requests, pictures) {
                Ok(()) => Start::HandedOver,
                // Couldn't leave the request: open here after all.
                Err(_) => Start::Running(Instance::alone()),
            },
        }
    }

    /// True for the Duck Quill that answers double-clicks.
    pub fn answers(&self) -> bool {
        self.lock.is_some()
    }

    /// Requests that came in, oldest first: each is the pictures to open
    /// (none: just come to the front). Looks at most every `LOOK_EVERY`.
    pub fn take_requests(&mut self) -> Vec<Vec<PathBuf>> {
        let Some(folder) = &self.requests else {
            return Vec::new();
        };
        if self.last_look.elapsed() < LOOK_EVERY {
            return Vec::new();
        }
        self.last_look = Instant::now();
        let Ok(entries) = std::fs::read_dir(folder) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "txt"))
            .collect();
        files.sort();
        files
            .into_iter()
            .filter_map(|file| {
                let text = std::fs::read_to_string(&file).ok()?;
                let _ = std::fs::remove_file(&file);
                Some(
                    text.lines()
                        .filter(|l| !l.is_empty())
                        .map(PathBuf::from)
                        .collect(),
                )
            })
            .collect()
    }
}

/// Deletes requests older than half a minute.
fn forget_old_requests(folder: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_none_or(|age| age > Duration::from_secs(30));
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Leaves the pictures for the running Duck Quill: written under another
/// name first, so it never reads half a request.
fn hand_over(folder: &Path, pictures: &[PathBuf]) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("{stamp:024}-{}", std::process::id());
    let partial = folder.join(format!("{name}.part"));
    let mut file = File::create(&partial)?;
    for picture in pictures {
        let full = crate::settings::full_path(picture);
        writeln!(file, "{}", full.display())?;
    }
    file.sync_all()?;
    drop(file);
    std::fs::rename(&partial, folder.join(format!("{name}.txt")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_start_hands_its_picture_to_the_first() {
        let folder = std::env::temp_dir().join(format!("quill-instance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let Start::Running(mut first) = Instance::start(Some(&folder), &[], false) else {
            panic!("the first one runs");
        };
        assert!(first.answers());
        let picture = folder.join("pond.quill");
        assert!(matches!(
            Instance::start(Some(&folder), std::slice::from_ref(&picture), false),
            Start::HandedOver
        ));
        // Started from the Start menu with no picture: just come forward.
        assert!(matches!(
            Instance::start(Some(&folder), &[], false),
            Start::HandedOver
        ));
        // File → New window: runs, but doesn't answer double-clicks.
        let Start::Running(second) = Instance::start(Some(&folder), &[], true) else {
            panic!("a new window runs");
        };
        assert!(!second.answers());

        first.last_look = Instant::now() - LOOK_EVERY;
        let requests = first.take_requests();
        assert_eq!(requests, vec![vec![picture.clone()], vec![]]);
        first.last_look = Instant::now() - LOOK_EVERY;
        assert!(first.take_requests().is_empty(), "each is answered once");

        // When the first closes, the next start runs and answers.
        drop(first);
        let Start::Running(next) = Instance::start(Some(&folder), &[picture], false) else {
            panic!("nobody is running");
        };
        assert!(next.answers());
        drop(next);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
