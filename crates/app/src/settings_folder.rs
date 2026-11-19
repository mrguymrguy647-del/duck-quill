//! Where Duck Quill keeps its own files (settings, presets, copies of
//! unsaved work): `%APPDATA%\Duck Quill` on Windows,
//! `~/.config/duck-quill` elsewhere.
//!
//! It also holds `program.txt`: where the Duck Quill program itself is.
//! Duck Quill has no installer, so it can be anywhere; its sister apps
//! (Duck Mold, Duck Forge) read this file to find it and start it.

use std::path::{Path, PathBuf};

/// The file in the settings folder that says where the program is.
pub const PROGRAM_FILE: &str = "program.txt";

/// The folder, or `None` if the system doesn't say where it should be.
/// It may not exist yet: whoever saves into it creates it.
pub fn settings_folder() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Duck Quill"))
    }
    #[cfg(not(windows))]
    {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|folder| folder.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(config.join("duck-quill"))
    }
}

/// Writes the running program's full path into `program.txt` in `folder`
/// (made if needed): one line of UTF-8 text, ending with a line break.
/// Done every time Duck Quill starts, so the file follows the program
/// when its folder is moved or replaced by a new version.
///
/// It's written under a temporary name and swapped in, so a program
/// reading it never sees half a path.
pub fn write_program_path(folder: &Path, program: &Path) -> std::io::Result<()> {
    // A path that isn't proper text can't be written as UTF-8 without
    // changing it, and a changed path would lead nowhere.
    let Some(text) = program.to_str() else {
        return Err(std::io::Error::other("the program's path isn't valid text"));
    };
    std::fs::create_dir_all(folder)?;
    let temporary = folder.join(format!("{PROGRAM_FILE}.tmp"));
    std::fs::write(&temporary, format!("{text}\n"))?;
    std::fs::rename(&temporary, folder.join(PROGRAM_FILE))
}

/// Notes where this running Duck Quill is, for its sister apps. If that
/// can't be done (no settings folder, a full disk), nothing happens:
/// Duck Quill works the same without it.
pub fn note_program_path(folder: Option<&Path>) {
    if let (Some(folder), Ok(program)) = (folder, std::env::current_exe()) {
        let _ = write_program_path(folder, &program);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_path_is_written_as_one_line() {
        let folder = std::env::temp_dir()
            .join(format!("quill-program-{}", std::process::id()))
            .join("Duck Quill");
        let _ = std::fs::remove_dir_all(&folder);
        // (A made-up place, with a space and an accent, as a real one may have.)
        let program = std::env::temp_dir()
            .join("Duck Quill é")
            .join("DuckQuill.exe");

        // The folder doesn't exist yet: it's made.
        write_program_path(&folder, &program).unwrap();
        let file = folder.join(PROGRAM_FILE);
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text, format!("{}\n", program.display()));
        assert_eq!(text.lines().count(), 1);
        assert_eq!(Path::new(text.trim_end()), program);
        assert!(Path::new(text.trim_end()).is_absolute());

        // Starting again from another place replaces it, and nothing
        // temporary stays behind.
        let moved = std::env::temp_dir().join("elsewhere").join("DuckQuill.exe");
        write_program_path(&folder, &moved).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text, format!("{}\n", moved.display()));
        let names: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [PROGRAM_FILE]);

        // The running program's own path is a full one, and goes in too.
        note_program_path(Some(&folder));
        let text = std::fs::read_to_string(&file).unwrap();
        let running = std::env::current_exe().unwrap();
        assert_eq!(Path::new(text.trim_end()), running);
        assert!(running.is_absolute());

        // A folder that can't be made is an error to ignore, not a crash.
        let in_the_way = folder.join(PROGRAM_FILE);
        assert!(write_program_path(&in_the_way, &program).is_err());
        note_program_path(Some(&in_the_way));
        note_program_path(None);
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }
}
