//! Where FERRITE keeps its files.
//!
//! Everything lives beside the program (a portable install), except in the
//! macOS app: an app bundle must not change itself, so `FERRITE.app` keeps
//! settings, maps, prefabs, music, screenshots and stats in
//! `~/Library/Application Support/FERRITE`.

use std::path::{Path, PathBuf};

/// True when `exe` is the program inside a macOS app bundle
/// (`Something.app/Contents/MacOS/program`).
pub fn in_app_bundle(exe: &Path) -> bool {
    let mut up = exe.ancestors().skip(1);
    match (up.next(), up.next(), up.next()) {
        (Some(macos), Some(contents), Some(app)) => {
            macos.file_name().is_some_and(|n| n == "MacOS")
                && contents.file_name().is_some_and(|n| n == "Contents")
                && app.extension().is_some_and(|e| e.eq_ignore_ascii_case("app"))
        }
        _ => false,
    }
}

/// Whether this program is running from a macOS app bundle.
pub fn bundled() -> bool {
    std::env::current_exe().is_ok_and(|e| in_app_bundle(&e))
}

/// The folder for the game's own files (see the module notes). Created if
/// it does not exist yet.
pub fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().ok();
    if exe.as_deref().is_some_and(in_app_bundle) {
        if let Some(home) = std::env::var_os("HOME") {
            let d = PathBuf::from(home).join("Library").join("Application Support").join("FERRITE");
            let _ = std::fs::create_dir_all(&d);
            return d;
        }
    }
    exe.and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_program_inside_an_app_bundle_counts_as_bundled() {
        assert!(in_app_bundle(Path::new("/Applications/FERRITE.app/Contents/MacOS/ferrite")));
        assert!(in_app_bundle(Path::new("/Users/a/Desktop/Ferrite.APP/Contents/MacOS/ferrite-server")));
        assert!(!in_app_bundle(Path::new("/Users/a/ferrite-v0.15.3-macos-universal/ferrite")));
        assert!(!in_app_bundle(Path::new("/Applications/FERRITE.app/Contents/Resources/ferrite")));
        assert!(!in_app_bundle(Path::new("/Contents/MacOS/ferrite")));
        assert!(!in_app_bundle(Path::new("ferrite")));
    }
}
