//! Updates from the project's GitHub releases.
//!
//! The launcher asks which release is the latest (a background thread; the
//! answer is the redirect of `/releases/latest`). If it is newer than this
//! build it offers to update: the build for this platform is downloaded,
//! unpacked, and its files put in place of the ones beside the executable.
//! The files being replaced are renamed to `*.old` first - a running
//! program can be renamed but not overwritten - and cleared away at the
//! next start. Settings and player records are never touched.
//!
//! Downloading is left to the `curl` and `tar` that Windows 10, macOS and
//! Linux all come with, which keeps a TLS stack out of the game. Without
//! them there is simply no update notice. Downloads are HTTPS from
//! github.com; nothing about the player is sent.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

pub const REPO: &str = "https://github.com/Kalaghni/ferrite";

/// The archive published for this platform, after `ferrite-<tag>-`.
#[cfg(target_os = "windows")]
const ASSET: &str = "windows-x64.zip";
#[cfg(target_os = "macos")]
const ASSET: &str = "macos-universal.tar.gz";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const ASSET: &str = "linux-x64.tar.gz";

/// Files a player may have edited: kept if they are already there.
const KEEP: [&str; 2] = ["start-server.bat", "start-server.sh"];

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Not asked, or told not to ask.
    Off,
    Checking,
    UpToDate,
    /// A newer release: its tag, e.g. "v0.7.1".
    Available(String),
    Installing(String),
    /// Installed; the game should restart.
    Installed(String),
    Failed(String),
}

enum Msg {
    Latest(Option<String>),
    Done(Result<(), String>),
}

pub struct Updater {
    pub state: State,
    rx: Option<Receiver<Msg>>,
    current: String,
    /// Captured before replacement: current_exe() may later point to *.old.
    restart_exe: Option<PathBuf>,
}

/// "v0.7.1" or "0.7.1" -> [0, 7, 1]. `None` if it is not a version.
pub fn parse_version(s: &str) -> Option<Vec<u32>> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next().unwrap_or("");
    let parts: Option<Vec<u32>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty() && p.len() <= 4)
}

/// Is release `latest` newer than version `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

/// The tag in the `Location:` line of the reply to `/releases/latest`.
pub fn tag_from_headers(headers: &str) -> Option<String> {
    let line = headers.lines().rev().find(|l| l.to_ascii_lowercase().starts_with("location:"))?;
    let tag = line.rsplit_once("/releases/tag/")?.1.trim();
    let ok = !tag.is_empty() && tag.len() <= 32 && tag.chars().all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c));
    (ok && parse_version(tag).is_some()).then(|| tag.to_string())
}

pub fn asset_url(tag: &str) -> String {
    format!("{REPO}/releases/download/{tag}/ferrite-{tag}-{ASSET}")
}

/// `curl` or `tar`: on Windows the ones in System32, whatever else is on
/// the path (another `tar` may not read zip files), and without a console
/// window flashing up.
fn tool(name: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let exe = Path::new(&sys).join("System32").join(format!("{name}.exe"));
        let mut c = Command::new(if exe.exists() { exe } else { PathBuf::from(name) });
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    }
    #[cfg(not(windows))]
    {
        Command::new(name)
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|d| d.to_path_buf())
}

/// A build run from cargo's `target` folder is someone working on the
/// game: it is not ours to replace.
fn dev_build() -> bool {
    exe_dir().is_some_and(|d| d.components().any(|c| c.as_os_str() == "target"))
}

/// Ask GitHub which release is the latest.
pub fn latest_tag() -> Option<String> {
    let out =
        tool("curl").args(["-fsSI", "--max-time", "8", "-A", "ferrite-updater"]).arg(format!("{REPO}/releases/latest")).output().ok()?;
    tag_from_headers(&String::from_utf8_lossy(&out.stdout))
}

/// Put the files of `from` in place of those in `to`. What is replaced is
/// renamed to `*.old` first, so it works on the running executable.
pub fn swap_in(from: &Path, to: &Path) -> Result<usize, String> {
    let mut n = 0;
    for e in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name();
        let src = e.path();
        if !src.is_file() {
            continue;
        }
        let dst = to.join(&name);
        if dst.exists() {
            if KEEP.iter().any(|k| name == std::ffi::OsStr::new(k)) {
                continue;
            }
            let mut old = dst.clone().into_os_string();
            old.push(".old");
            let old = PathBuf::from(old);
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&dst, &old).map_err(|e| format!("cannot replace {}: {e}", dst.display()))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| format!("cannot write {}: {e}", dst.display()))?;
        n += 1;
    }
    Ok(n)
}

/// Clear away what the last update left behind.
pub fn clean_old() {
    let Some(dir) = exe_dir() else { return };
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let ours = p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".old") && (n.starts_with("ferrite") || n.starts_with("README") || n.starts_with("Quantico")));
        if ours && p.is_file() {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Download release `tag` and put it in place of the files in `dir`.
pub fn install(tag: &str, dir: &Path) -> Result<(), String> {
    // Find out now whether the folder can be written to at all.
    let probe = dir.join(".ferrite-update-probe");
    std::fs::write(&probe, b"").map_err(|_| format!("cannot write to {}", dir.display()))?;
    let _ = std::fs::remove_file(&probe);

    let tmp = std::env::temp_dir().join(format!("ferrite-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        let archive = tmp.join(format!("ferrite-{tag}-{ASSET}"));
        let got =
            tool("curl").args(["-fsSL", "--max-time", "600", "-A", "ferrite-updater", "-o"]).arg(&archive).arg(asset_url(tag)).status();
        match got {
            Ok(s) if s.success() => {}
            Ok(_) => return Err(format!("could not download {tag} for this system")),
            Err(_) => return Err("curl is not available".to_string()),
        }
        let unpacked = tmp.join("unpacked");
        std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        let ok = tool("tar").arg("-xf").arg(&archive).arg("-C").arg(&unpacked).status().map_err(|_| "tar is not available".to_string())?;
        if !ok.success() {
            return Err("could not unpack the download".to_string());
        }
        // The archive holds one folder, named after the release.
        let inner = std::fs::read_dir(&unpacked)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_dir())
            .unwrap_or(unpacked.clone());
        let exe = if cfg!(windows) { "ferrite.exe" } else { "ferrite" };
        if !inner.join(exe).is_file() {
            return Err("the download does not look like FERRITE".to_string());
        }
        swap_in(&inner, dir).map(|_| ())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// The release always installs this name, even if the running image was renamed.
fn installed_executable(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) { "ferrite.exe" } else { "ferrite" })
}

/// Update-only arguments must not force another update after a successful restart.
fn restart_args(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut out = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--pretend-version" {
            let _ = args.next();
        } else if arg != "--update" && !arg.to_str().is_some_and(|s| s.starts_with("--pretend-version=")) {
            out.push(arg);
        }
    }
    out
}

impl Updater {
    /// Start looking for a newer release, unless told not to. `pretend`
    /// stands in for this build's version (for trying the updater out).
    pub fn new(enabled: bool, pretend: Option<&str>) -> Updater {
        let current = pretend.unwrap_or(env!("CARGO_PKG_VERSION")).to_string();
        if !enabled || (dev_build() && pretend.is_none()) {
            return Updater { state: State::Off, rx: None, current, restart_exe: None };
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Latest(latest_tag()));
        });
        Updater { state: State::Checking, rx: Some(rx), current, restart_exe: None }
    }

    /// Take in whatever the background work has reported.
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            self.state = match (m, &self.state) {
                (Msg::Latest(Some(tag)), _) if is_newer(&tag, &self.current) => State::Available(tag),
                (Msg::Latest(_), _) => State::UpToDate,
                (Msg::Done(Ok(())), State::Installing(tag)) => State::Installed(tag.clone()),
                (Msg::Done(Err(e)), _) => State::Failed(e),
                (Msg::Done(Ok(())), s) => s.clone(),
            };
        }
    }

    /// Download and install the release that was found.
    pub fn install(&mut self) {
        let State::Available(tag) = &self.state else { return };
        let tag = tag.clone();
        let Some(dir) = exe_dir() else {
            self.state = State::Failed("cannot locate the installation directory".into());
            return;
        };
        // Resolve the new executable before the worker renames the running image.
        self.restart_exe = Some(installed_executable(&dir));
        let (tx, rx) = channel();
        let t = tag.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Done(install(&t, &dir)));
        });
        self.rx = Some(rx);
        self.state = State::Installing(tag);
    }

    /// Start the installed executable. The caller exits through normal cleanup
    /// on success, or keeps the launcher visible if spawning fails.
    pub fn restart(&mut self) -> bool {
        self.launch_installed(std::env::args_os().skip(1), |command| command.spawn().map(|_| ()))
    }

    fn launch_installed(
        &mut self,
        args: impl IntoIterator<Item = OsString>,
        spawn: impl FnOnce(&mut Command) -> std::io::Result<()>,
    ) -> bool {
        if !matches!(self.state, State::Installed(_)) {
            return false;
        }
        let Some(exe) = &self.restart_exe else {
            self.state = State::Failed("installed update, but its restart path is unavailable; reopen FERRITE".into());
            return false;
        };
        let mut command = Command::new(exe);
        command.args(restart_args(args));
        match spawn(&mut command) {
            Ok(()) => true,
            Err(e) => {
                self.state = State::Failed(format!("installed update, but could not start {}: {e}; reopen FERRITE", exe.display()));
                false
            }
        }
    }
}

/// `ferrite --update`: check, install and report on the command line.
/// `pretend` stands in for this build's version (for trying it out).
pub fn run_cli(pretend: Option<&str>) -> i32 {
    let current = pretend.unwrap_or(env!("CARGO_PKG_VERSION"));
    let Some(tag) = latest_tag() else {
        println!("could not reach {REPO}/releases (is curl installed?)");
        return 1;
    };
    if !is_newer(&tag, current) {
        println!("FERRITE {current} is up to date (latest release: {tag})");
        return 0;
    }
    let Some(dir) = exe_dir() else { return 1 };
    println!("updating FERRITE {current} to {tag} in {} ...", dir.display());
    match install(&tag, &dir) {
        Ok(()) => {
            println!("updated to {tag}");
            0
        }
        Err(e) => {
            println!("update failed: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.7.1", "0.7.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("v1.0", "0.99.99"));
        assert!(!is_newer("v0.7.0", "0.7.0"));
        assert!(!is_newer("v0.6.0", "0.7.0"));
        assert!(!is_newer("v0.7", "0.7.0"));
        assert!(!is_newer("nightly", "0.7.0"));
        assert!(!is_newer("", "0.7.0"));
        assert_eq!(parse_version("v0.7.1-rc1"), Some(vec![0, 7, 1]));
        assert_eq!(parse_version("v1.x"), None);
    }

    #[test]
    fn the_latest_tag_is_read_from_the_redirect() {
        let h = "HTTP/2 302 \r\nserver: GitHub.com\r\nLocation: https://github.com/Kalaghni/ferrite/releases/tag/v0.7.1\r\ncontent-length: 0\r\n\r\n";
        assert_eq!(tag_from_headers(h), Some("v0.7.1".to_string()));
        assert_eq!(tag_from_headers("HTTP/2 404\r\n\r\n"), None);
        // Nothing but a plain tag is accepted: it goes into a URL and a file name.
        assert_eq!(tag_from_headers("location: https://github.com/a/b/releases/tag/v1.0/../../evil\r\n"), None);
        assert_eq!(tag_from_headers("location: https://github.com/a/b/releases/tag/v1.0 && calc\r\n"), None);
        assert!(asset_url("v0.7.1").starts_with("https://github.com/Kalaghni/ferrite/releases/download/v0.7.1/ferrite-v0.7.1-"));
    }

    #[test]
    fn restart_discards_update_and_version_override_arguments() {
        let args = ["--fullscreen", "--pretend-version", "0.5.0", "--update", "--name", "Pilot", "--pretend-version=0.6.0"];
        let actual = restart_args(args.map(OsString::from));
        assert_eq!(actual, ["--fullscreen", "--name", "Pilot"].map(OsString::from));
        assert!(restart_args([OsString::from("--pretend-version")]).is_empty());
    }

    #[test]
    fn restart_uses_the_installed_path_after_the_running_image_is_renamed() {
        let dir = std::env::temp_dir().join("ferrite-restart-path-test");
        let installed = installed_executable(&dir);
        let mut running_image = installed.clone().into_os_string();
        running_image.push(".old");
        let running_image = PathBuf::from(running_image);
        let mut updater =
            Updater { state: State::Installed("v0.8.1".into()), rx: None, current: "0.7.0".into(), restart_exe: Some(installed.clone()) };
        assert!(updater.launch_installed([OsString::from("--fullscreen")], |command| {
            assert_eq!(command.get_program(), installed.as_os_str());
            assert_ne!(command.get_program(), running_image.as_os_str());
            assert_eq!(command.get_args().collect::<Vec<_>>(), [std::ffi::OsStr::new("--fullscreen")]);
            Ok(())
        }));
    }

    #[test]
    fn restart_failure_keeps_the_launcher_open_with_an_error() {
        let mut updater = Updater {
            state: State::Installed("v0.8.1".into()),
            rx: None,
            current: "0.7.0".into(),
            restart_exe: Some(installed_executable(Path::new("installed"))),
        };
        assert!(!updater.launch_installed([], |_| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "blocked executable"))));
        let State::Failed(message) = &updater.state else { panic!("restart failure was not shown") };
        assert!(message.contains("blocked executable"));
        assert!(message.contains("reopen FERRITE"));
        // The next frame must not attempt another automatic restart.
        assert!(!updater.launch_installed([], |_| panic!("retried a failed restart")));
    }

    #[test]
    fn files_are_swapped_in_and_the_old_ones_kept_aside() {
        let base = std::env::temp_dir().join(format!("ferrite-swap-test-{}", std::process::id()));
        let (new, live) = (base.join("new"), base.join("live"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&live).unwrap();
        for (name, text) in
            [("ferrite.exe", "new game"), ("README.txt", "new readme"), ("start-server.bat", "new bat"), ("extra.txt", "extra")]
        {
            std::fs::write(new.join(name), text).unwrap();
        }
        for (name, text) in [("ferrite.exe", "old game"), ("start-server.bat", "my own options"), ("ferrite-client.txt", "name=me")] {
            std::fs::write(live.join(name), text).unwrap();
        }
        assert_eq!(swap_in(&new, &live), Ok(3));
        let read = |n: &str| std::fs::read_to_string(live.join(n)).unwrap();
        assert_eq!(read("ferrite.exe"), "new game");
        assert_eq!(read("ferrite.exe.old"), "old game");
        assert_eq!(read("README.txt"), "new readme");
        assert_eq!(read("extra.txt"), "extra");
        // Edited by the player, and their settings: left as they were.
        assert_eq!(read("start-server.bat"), "my own options");
        assert_eq!(read("ferrite-client.txt"), "name=me");
        let _ = std::fs::remove_dir_all(&base);
    }
}
