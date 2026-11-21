//! Updates from the project's GitHub releases.
//!
//! The launcher asks which release is the latest (a background thread; the
//! answer is the redirect of `/releases/latest`). If it is newer than this
//! build it offers to update. Settings and player records are never touched.
//!
//! Installing, step by step:
//!
//! 1. Ask GitHub's release API for the release's files and their sizes. The
//!    release must have this platform's package and a `SHA256SUMS` list.
//! 2. Download `SHA256SUMS` and the package (HTTPS only, from github.com and
//!    the hosts it redirects release files to). The package must have the
//!    size the API reported and the SHA-256 the list gives for it, or
//!    nothing is installed (see [`verify`]).
//! 3. Unpack it, copy the files to `ferrite-staging/` beside the executable,
//!    and swap them in: each installed file is renamed into
//!    `ferrite-previous/`, then the new one renamed into its place. A record
//!    of the moves is written first, and if any step fails every file is
//!    put back (see [`swap`]).
//! 4. Start the new version. The record says the update is pending; the
//!    new version counts its first start there, and once its launcher has
//!    drawn a few frames (or a direct launch has finished normally) it marks
//!    the update confirmed.
//! 5. If a start finds a pending update that was already started once and
//!    never confirmed - it crashed or would not open - the previous version
//!    is put back, started in its place, and its launcher says so. A record
//!    left half way through a swap (the game killed mid-update) is undone the
//!    same way.
//!
//! The previous version stays in `ferrite-previous/` until the next update
//! has been downloaded and checked; that is when it is deleted. Until then
//! SETTINGS > VIDEO has a ROLL BACK button, and `ferrite --rollback` does the
//! same from the command line.
//!
//! The Windows installer (Inno Setup) and the zip install the same files in
//! one folder, and the updater treats both the same: it replaces those files
//! from the zip. The installer's uninstaller still works, and also removes
//! the updater's folders (`packaging/ferrite.iss`).
//!
//! Downloading is left to the `curl` and `tar` that Windows 10, macOS and
//! Linux all come with, which keeps a TLS stack out of the game. Without
//! them there is simply no update notice. Nothing about the player is sent.

mod swap;
mod verify;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

use swap::{Journal, Status};

pub const REPO: &str = "https://github.com/Kalaghni/ferrite";
const API: &str = "https://api.github.com/repos/Kalaghni/ferrite";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The archive published for this platform, after `ferrite-<tag>-`.
#[cfg(target_os = "windows")]
const ASSET: &str = "windows-x64.zip";
#[cfg(target_os = "macos")]
const ASSET: &str = "macos-universal.tar.gz";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const ASSET: &str = "linux-x64.tar.gz";

/// A message for the next launcher to show (a rollback happened), beside
/// the executable. Read and deleted when the launcher opens.
const NOTICE: &str = "ferrite-update-notice.txt";

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
    /// The previous version (e.g. "0.12.0") is back; the game should restart.
    RolledBack(String),
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
    /// Captured before replacement: current_exe() may later point elsewhere.
    restart_exe: Option<PathBuf>,
    /// The version kept from before the last update, if it can be rolled back to.
    pub previous: Option<String>,
    /// What the last start did that the player should know (a rollback).
    pub notice: Option<String>,
}

/// "v0.7.1" or "0.7.1" -> [0, 7, 1]. `None` if it is not a version.
pub fn parse_version(s: &str) -> Option<Vec<u32>> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next().unwrap_or("");
    let parts: Option<Vec<u32>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty() && p.len() <= 4)
}

fn compare(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let (mut a, mut b) = (parse_version(a)?, parse_version(b)?);
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    Some(a.cmp(&b))
}

/// Is release `latest` newer than version `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    compare(latest, current) == Some(std::cmp::Ordering::Greater)
}

/// "v0.12.0" and "0.12" are the same version.
fn same_version(a: &str, b: &str) -> bool {
    compare(a, b) == Some(std::cmp::Ordering::Equal)
}

/// The tag in the `Location:` line of the reply to `/releases/latest`.
pub fn tag_from_headers(headers: &str) -> Option<String> {
    let line = headers.lines().rev().find(|l| l.to_ascii_lowercase().starts_with("location:"))?;
    let tag = line.rsplit_once("/releases/tag/")?.1.trim();
    let ok = !tag.is_empty() && tag.len() <= 32 && tag.chars().all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c));
    (ok && parse_version(tag).is_some()).then(|| tag.to_string())
}

/// This platform's package in release `tag`.
pub fn asset_name(tag: &str) -> String {
    format!("ferrite-{tag}-{ASSET}")
}

/// Where a file of release `tag` is downloaded from.
pub fn download_url(tag: &str, file: &str) -> String {
    format!("{REPO}/releases/download/{tag}/{file}")
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

/// `curl` for HTTPS only: it refuses a plain-HTTP address, and a redirect to
/// one, rather than following it.
fn curl() -> Command {
    let mut c = tool("curl");
    c.args(["--proto", "=https", "--proto-redir", "=https", "--max-redirs", "8", "-A", "ferrite-updater"]);
    c
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
    let out = curl().args(["-fsSI", "--max-time", "8"]).arg(format!("{REPO}/releases/latest")).output().ok()?;
    tag_from_headers(&String::from_utf8_lossy(&out.stdout))
}

/// Where the public server list is published.
pub const PUBLIC_LIST: &str = "https://playferrite.com/servers.json";

/// The public server list as published, if the site answers.
pub fn public_list() -> Option<String> {
    let out = curl().args(["-fsSL", "--max-time", "6", "--max-filesize", "65536"]).arg(PUBLIC_LIST).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The files of release `tag` and their sizes, from GitHub's release API.
fn release_assets(tag: &str) -> Result<Vec<verify::Asset>, String> {
    let out = curl()
        .args(["-fsSL", "--max-time", "20", "--max-filesize", "4000000", "-H", "Accept: application/vnd.github+json"])
        .arg(format!("{API}/releases/tags/{tag}"))
        .output()
        .map_err(|_| "curl is not available".to_string())?;
    if !out.status.success() {
        return Err(format!("could not ask GitHub about {tag}; try again later"));
    }
    verify::parse_assets(&String::from_utf8_lossy(&out.stdout))
}

/// Download `url` to `out`. Refused if it is bigger than `size` bytes, or if
/// it was not served over HTTPS from GitHub after any redirects.
fn download(url: &str, out: &Path, size: u64) -> Result<(), String> {
    let file = url.rsplit('/').next().unwrap_or(url);
    if !verify::allowed_url(url) {
        return Err(format!("will not download from {url}"));
    }
    let got = curl()
        .args(["-fsSL", "--max-time", "600", "--max-filesize"])
        .arg(size.to_string())
        .args(["-w", "%{url_effective}", "-o"])
        .arg(out)
        .arg(url)
        .output()
        .map_err(|_| "curl is not available".to_string())?;
    if !got.status.success() {
        let _ = std::fs::remove_file(out);
        return Err(match got.status.code() {
            Some(63) => format!("{file} is bigger than the release says; not installing"),
            Some(1) => format!("{file} was offered over a connection that is not HTTPS; not installing"),
            _ => format!("could not download {file}"),
        });
    }
    let landed = String::from_utf8_lossy(&got.stdout).trim().to_string();
    if !verify::allowed_url(&landed) {
        let _ = std::fs::remove_file(out);
        return Err(format!("{file} came from {landed}, not GitHub; not installing"));
    }
    Ok(())
}

/// Clear away what the updater of 0.11 and earlier left behind (`*.old`).
fn clean_old(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
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

/// Download release `tag`, check it, and put it in place of the files in `dir`.
pub fn install(tag: &str, dir: &Path) -> Result<(), String> {
    // Find out now whether the folder can be written to at all.
    let probe = dir.join(".ferrite-update-probe");
    std::fs::write(&probe, b"").map_err(|_| format!("cannot write to {}", dir.display()))?;
    let _ = std::fs::remove_file(&probe);

    let tmp = std::env::temp_dir().join(format!("ferrite-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        let name = asset_name(tag);
        let assets = release_assets(tag)?;
        let package = assets.iter().find(|a| a.name == name).ok_or_else(|| format!("{tag} has no build for this system"))?;
        let sums = assets
            .iter()
            .find(|a| a.name == verify::SUMS)
            .ok_or_else(|| format!("{tag} has no {} to check the download against; not installing", verify::SUMS))?;
        if sums.size == 0 || sums.size > verify::SUMS_MAX {
            return Err(format!("{tag}'s {} is not a checksum list; not installing", verify::SUMS));
        }

        let sums_file = tmp.join(verify::SUMS);
        download(&download_url(tag, verify::SUMS), &sums_file, sums.size)?;
        let text = std::fs::read(&sums_file).map_err(|e| e.to_string())?;
        if text.len() as u64 != sums.size {
            return Err(format!("{} did not download completely; not installing", verify::SUMS));
        }
        let want = verify::expected_sum(&String::from_utf8_lossy(&text), &name)?;

        let archive = tmp.join(&name);
        download(&download_url(tag, &name), &archive, package.size)?;
        verify::check_download(&archive, package.size, &want)?;

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
        put_in_place(&inner, dir, tag)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Put the unpacked release `tag` in place of the files in `dir`, keeping
/// the files it replaces. The version kept from the update before is
/// deleted here: only one previous version is kept.
fn put_in_place(unpacked: &Path, dir: &Path, tag: &str) -> Result<(), String> {
    let exe = installed_executable(Path::new(""));
    if !unpacked.join(&exe).is_file() {
        return Err("the download does not look like FERRITE".to_string());
    }
    // Copy onto the install's own volume first, so the swap is all renames.
    let staging = dir.join(swap::STAGING);
    swap::discard(&staging, dir)?;
    std::fs::create_dir(&staging).map_err(|e| format!("cannot create {}: {e}", staging.display()))?;
    let result = (|| {
        for e in std::fs::read_dir(unpacked).map_err(|e| e.to_string())?.flatten() {
            if e.path().is_file() {
                let to = staging.join(e.file_name());
                std::fs::copy(e.path(), &to).map_err(|err| format!("cannot write {}: {err}", to.display()))?;
            }
        }
        swap::drop_previous(dir)?;
        swap::swap(&staging, dir, VERSION, tag).map(|_| ())
    })();
    let _ = swap::discard(&staging, dir);
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
        } else if arg != "--update" && arg != "--rollback" && !arg.to_str().is_some_and(|s| s.starts_with("--pretend-version=")) {
            out.push(arg);
        }
    }
    out
}

/// What a start found in the update record.
#[derive(Debug, PartialEq)]
enum Begun {
    /// No update in progress.
    Nothing,
    /// The first start of a new version: counted, waiting for it to confirm.
    Trying,
    /// The previous version was put back, from this record.
    Restored(Journal),
    /// It needed putting back, and that failed.
    Stuck(String),
}

/// Look at the update record in `dir` as version `running` starts.
fn begin(dir: &Path, running: &str) -> Begun {
    swap::empty_trash(dir);
    let Some(mut j) = Journal::load(dir) else { return Begun::Nothing };
    let restore = |j: &Journal| match swap::restore(dir) {
        Ok(_) => Begun::Restored(j.clone()),
        Err(e) => Begun::Stuck(format!("could not put {} back after {} failed: {e}", j.from, j.to)),
    };
    match j.status {
        // Killed half way through a swap.
        Status::Swapping => restore(&j),
        // Something else (the installer, by hand) put another version here:
        // the kept files no longer go with what is installed.
        Status::Pending | Status::Confirmed if !same_version(&j.to, running) => {
            let _ = swap::drop_previous(dir);
            Begun::Nothing
        }
        // Started before and never said it was fine.
        Status::Pending if j.launches >= 1 => restore(&j),
        Status::Pending => {
            j.launches += 1;
            let _ = j.save(dir);
            Begun::Trying
        }
        Status::Confirmed => Begun::Nothing,
    }
}

/// Mark a pending update to version `running` as working.
fn confirm(dir: &Path, running: &str) {
    if let Some(mut j) = Journal::load(dir) {
        if j.status == Status::Pending && same_version(&j.to, running) {
            j.status = Status::Confirmed;
            let _ = j.save(dir);
        }
    }
}

/// The version kept from before the update to `running`, if there is one.
fn previous_version(dir: &Path, running: &str) -> Option<String> {
    let j = Journal::load(dir)?;
    (j.status != Status::Swapping && same_version(&j.to, running)).then_some(j.from)
}

fn write_notice(dir: &Path, text: &str) {
    let _ = std::fs::write(dir.join(NOTICE), text);
}

fn take_notice(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(NOTICE)).ok()?;
    let _ = std::fs::remove_file(dir.join(NOTICE));
    let text: String = text.lines().next().unwrap_or("").chars().take(160).collect();
    (!text.trim().is_empty()).then_some(text)
}

/// Every windowed start, before the window opens: tidy up after the last
/// update and, if it never got going, put the previous version back. When
/// that replaced this very program, the restored one is started in its
/// place and this process ends here.
pub fn on_start() {
    let Some(dir) = exe_dir() else { return };
    clean_old(&dir);
    match begin(&dir, VERSION) {
        Begun::Restored(j) => {
            let text = if j.status == Status::Swapping {
                format!("The update to {} was interrupted, so {} was kept.", j.to, j.from)
            } else {
                format!("FERRITE {} did not start properly, so {} was put back.", j.to, j.from)
            };
            eprintln!("{text}");
            write_notice(&dir, &text);
            if !same_version(&j.from, VERSION) {
                let exe = installed_executable(&dir);
                match Command::new(&exe).args(restart_args(std::env::args_os().skip(1))).spawn() {
                    Ok(_) => std::process::exit(0),
                    Err(e) => write_notice(&dir, &format!("{text} Could not start it ({e}): reopen FERRITE.")),
                }
            }
        }
        Begun::Stuck(e) => {
            eprintln!("{e}");
            write_notice(&dir, &e);
        }
        Begun::Trying | Begun::Nothing => {}
    }
}

/// The new version is up and running: the update is good. Called once the
/// launcher has drawn a few frames, or a direct launch has ended normally.
pub fn confirm_start() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        if let Some(dir) = exe_dir() {
            confirm(&dir, VERSION);
        }
    }
}

impl Updater {
    /// Does nothing and touches no files (a placeholder until the launcher opens).
    pub fn off() -> Updater {
        Updater { state: State::Off, rx: None, current: VERSION.into(), restart_exe: None, previous: None, notice: None }
    }

    /// Start looking for a newer release, unless told not to. `pretend`
    /// stands in for this build's version (for trying the updater out).
    /// Also picks up the kept previous version and any notice from the last start.
    pub fn new(enabled: bool, pretend: Option<&str>) -> Updater {
        let mut u = Updater::off();
        u.current = pretend.unwrap_or(VERSION).to_string();
        if let Some(dir) = exe_dir() {
            u.previous = previous_version(&dir, VERSION);
            u.notice = take_notice(&dir);
        }
        if !enabled || (dev_build() && pretend.is_none()) {
            return u;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Latest(latest_tag()));
        });
        u.state = State::Checking;
        u.rx = Some(rx);
        u
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

    /// Put the kept previous version back (the launcher's ROLL BACK).
    pub fn roll_back(&mut self) {
        if matches!(self.state, State::Installing(_)) {
            return;
        }
        let Some(dir) = exe_dir() else {
            self.state = State::Failed("cannot locate the installation directory".into());
            return;
        };
        self.restart_exe = Some(installed_executable(&dir));
        match swap::restore(&dir) {
            Ok(j) => {
                write_notice(&dir, &format!("Rolled back from {} to {}.", j.to, j.from));
                self.previous = None;
                self.state = State::RolledBack(j.from);
            }
            Err(e) => self.state = State::Failed(format!("could not roll back: {e}")),
        }
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
        if !matches!(self.state, State::Installed(_) | State::RolledBack(_)) {
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
                self.state = State::Failed(format!("installed, but could not start {}: {e}; reopen FERRITE", exe.display()));
                false
            }
        }
    }
}

/// `ferrite --update`: check, install and report on the command line.
/// `pretend` stands in for this build's version (for trying it out).
pub fn run_cli(pretend: Option<&str>) -> i32 {
    let current = pretend.unwrap_or(VERSION);
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
            println!("updated to {tag}; {VERSION} is kept in {} (ferrite --rollback)", dir.join(swap::PREVIOUS).display());
            0
        }
        Err(e) => {
            println!("update failed: {e}");
            1
        }
    }
}

/// `ferrite --rollback`: put the version kept by the last update back.
pub fn run_rollback_cli() -> i32 {
    let Some(dir) = exe_dir() else { return 1 };
    if Journal::load(&dir).is_none() {
        println!("no previous version is kept in {} (nothing in {})", dir.display(), swap::record_path(&dir).display());
        return 1;
    }
    match swap::restore(&dir) {
        Ok(j) => {
            let text = format!("Rolled back from {} to {}.", j.to, j.from);
            println!("{text}");
            write_notice(&dir, &text);
            0
        }
        Err(e) => {
            println!("roll back failed: {e}");
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
        assert!(same_version("v0.12.0", "0.12"));
        assert!(!same_version("v0.12.1", "0.12.0"));
        assert!(!same_version("nightly", "nightly"));
    }

    #[test]
    fn the_latest_tag_is_read_from_the_redirect() {
        let h = "HTTP/2 302 \r\nserver: GitHub.com\r\nLocation: https://github.com/Kalaghni/ferrite/releases/tag/v0.7.1\r\ncontent-length: 0\r\n\r\n";
        assert_eq!(tag_from_headers(h), Some("v0.7.1".to_string()));
        assert_eq!(tag_from_headers("HTTP/2 404\r\n\r\n"), None);
        // Nothing but a plain tag is accepted: it goes into a URL and a file name.
        assert_eq!(tag_from_headers("location: https://github.com/a/b/releases/tag/v1.0/../../evil\r\n"), None);
        assert_eq!(tag_from_headers("location: https://github.com/a/b/releases/tag/v1.0 && calc\r\n"), None);
        let url = download_url("v0.7.1", &asset_name("v0.7.1"));
        assert!(url.starts_with("https://github.com/Kalaghni/ferrite/releases/download/v0.7.1/ferrite-v0.7.1-"));
        assert!(verify::allowed_url(&url));
        assert!(verify::allowed_url(&download_url("v0.7.1", verify::SUMS)));
        assert!(verify::allowed_url(&format!("{API}/releases/tags/v0.7.1")));
    }

    #[test]
    fn restart_discards_update_and_version_override_arguments() {
        let args = ["--fullscreen", "--pretend-version", "0.5.0", "--update", "--name", "Pilot", "--pretend-version=0.6.0", "--rollback"];
        let actual = restart_args(args.map(OsString::from));
        assert_eq!(actual, ["--fullscreen", "--name", "Pilot"].map(OsString::from));
        assert!(restart_args([OsString::from("--pretend-version")]).is_empty());
    }

    fn installed(state: State, exe: PathBuf) -> Updater {
        let mut u = Updater::off();
        u.state = state;
        u.current = "0.7.0".into();
        u.restart_exe = Some(exe);
        u
    }

    #[test]
    fn restart_uses_the_installed_path_after_the_running_image_is_renamed() {
        let dir = std::env::temp_dir().join("ferrite-restart-path-test");
        let installed_exe = installed_executable(&dir);
        let running_image = dir.join(swap::PREVIOUS).join(installed_exe.file_name().unwrap());
        for state in [State::Installed("v0.8.1".into()), State::RolledBack("0.7.0".into())] {
            let mut updater = installed(state, installed_exe.clone());
            assert!(updater.launch_installed([OsString::from("--fullscreen")], |command| {
                assert_eq!(command.get_program(), installed_exe.as_os_str());
                assert_ne!(command.get_program(), running_image.as_os_str());
                assert_eq!(command.get_args().collect::<Vec<_>>(), [std::ffi::OsStr::new("--fullscreen")]);
                Ok(())
            }));
        }
    }

    #[test]
    fn restart_failure_keeps_the_launcher_open_with_an_error() {
        let mut updater = installed(State::Installed("v0.8.1".into()), installed_executable(Path::new("installed")));
        assert!(!updater.launch_installed([], |_| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "blocked executable"))));
        let State::Failed(message) = &updater.state else { panic!("restart failure was not shown") };
        assert!(message.contains("blocked executable"));
        assert!(message.contains("reopen FERRITE"));
        // The next frame must not attempt another automatic restart.
        assert!(!updater.launch_installed([], |_| panic!("retried a failed restart")));
    }

    /// A folder for one test, removed afterwards.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("ferrite-update-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(p.join("release")).unwrap();
            std::fs::create_dir_all(p.join("install")).unwrap();
            Tmp(p)
        }
        fn release(&self) -> PathBuf {
            self.0.join("release")
        }
        fn install(&self) -> PathBuf {
            self.0.join("install")
        }
        fn read(&self, name: &str) -> Option<String> {
            std::fs::read_to_string(self.install().join(name)).ok()
        }
        fn put(dir: &Path, files: &[(&str, &str)]) {
            for (n, t) in files {
                std::fs::write(dir.join(n), t).unwrap();
            }
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn game() -> String {
        installed_executable(Path::new("")).to_string_lossy().into_owned()
    }

    /// Install "release" (game text `new`) over an install of `old`.
    fn setup(t: &Tmp, old: &str, new: &str) {
        let g = game();
        Tmp::put(&t.install(), &[(&g, old), ("README.txt", "old readme"), ("start-server.bat", "mine"), ("ferrite-client.txt", "name=me")]);
        Tmp::put(&t.release(), &[(&g, new), ("README.txt", "new readme"), ("start-server.bat", "theirs")]);
    }

    #[test]
    fn an_update_keeps_the_previous_version_until_the_next_one() {
        let t = Tmp::new("keep");
        let g = game();
        setup(&t, "0.12 game", "0.13 game");
        put_in_place(&t.release(), &t.install(), "v0.13.0").unwrap();
        assert_eq!(t.read(&g).as_deref(), Some("0.13 game"));
        assert_eq!(t.read(&format!("{}/{g}", swap::PREVIOUS)).as_deref(), Some("0.12 game"));
        assert_eq!(t.read("start-server.bat").as_deref(), Some("mine"));
        assert_eq!(t.read("ferrite-client.txt").as_deref(), Some("name=me"));
        assert!(!t.install().join(swap::STAGING).exists());
        assert_eq!(previous_version(&t.install(), "0.13.0").as_deref(), Some(VERSION));

        // The next update replaces the kept version: only one is kept.
        Tmp::put(&t.release(), &[(&g, "0.14 game")]);
        put_in_place(&t.release(), &t.install(), "v0.14.0").unwrap();
        assert_eq!(t.read(&g).as_deref(), Some("0.14 game"));
        assert_eq!(t.read(&format!("{}/{g}", swap::PREVIOUS)).as_deref(), Some("0.13 game"));
    }

    #[test]
    fn something_that_is_not_ferrite_is_not_installed() {
        let t = Tmp::new("notgame");
        setup(&t, "0.12 game", "0.13 game");
        std::fs::remove_file(t.release().join(game())).unwrap();
        assert!(put_in_place(&t.release(), &t.install(), "v0.13.0").unwrap_err().contains("does not look like FERRITE"));
        assert_eq!(t.read(&game()).as_deref(), Some("0.12 game"));
        assert!(!t.install().join(swap::PREVIOUS).exists());
    }

    #[test]
    fn a_new_version_that_starts_is_confirmed_and_kept() {
        let t = Tmp::new("confirm");
        let dir = t.install();
        setup(&t, "0.12 game", "0.13 game");
        put_in_place(&t.release(), &dir, "v0.13.0").unwrap();
        assert_eq!(begin(&dir, "0.13.0"), Begun::Trying);
        assert_eq!(Journal::load(&dir).unwrap().launches, 1);
        confirm(&dir, "0.13.0");
        assert_eq!(Journal::load(&dir).unwrap().status, Status::Confirmed);
        // Later starts change nothing, crash or not.
        assert_eq!(begin(&dir, "0.13.0"), Begun::Nothing);
        assert_eq!(begin(&dir, "0.13.0"), Begun::Nothing);
        assert_eq!(t.read(&game()).as_deref(), Some("0.13 game"));
        assert!(previous_version(&dir, "0.13.0").is_some());
    }

    #[test]
    fn a_new_version_that_never_confirms_is_rolled_back() {
        let t = Tmp::new("crash");
        let dir = t.install();
        setup(&t, "0.12 game", "0.13 game");
        put_in_place(&t.release(), &dir, "v0.13.0").unwrap();
        assert_eq!(begin(&dir, "0.13.0"), Begun::Trying);
        // ... it crashed before confirming. The next start puts 0.12 back.
        let Begun::Restored(j) = begin(&dir, "0.13.0") else { panic!("not rolled back") };
        assert_eq!((j.from.as_str(), j.to.as_str(), j.status), (VERSION, "v0.13.0", Status::Pending));
        assert_eq!(t.read(&game()).as_deref(), Some("0.12 game"));
        assert_eq!(t.read("README.txt").as_deref(), Some("old readme"));
        assert_eq!(t.read("start-server.bat").as_deref(), Some("mine"));
        assert_eq!(t.read("ferrite-client.txt").as_deref(), Some("name=me"));
        assert!(!dir.join(swap::PREVIOUS).exists());
        assert_eq!(begin(&dir, VERSION), Begun::Nothing);
    }

    #[test]
    fn a_manual_rollback_works_after_confirming() {
        let t = Tmp::new("manual");
        let dir = t.install();
        setup(&t, "0.12 game", "0.13 game");
        put_in_place(&t.release(), &dir, "v0.13.0").unwrap();
        begin(&dir, "0.13.0");
        confirm(&dir, "0.13.0");
        swap::restore(&dir).unwrap();
        assert_eq!(t.read(&game()).as_deref(), Some("0.12 game"));
        assert_eq!(previous_version(&dir, VERSION), None);
    }

    #[test]
    fn a_record_for_another_version_is_dropped() {
        // The installer put 0.14 over a 0.13 update: the kept 0.12 files
        // are no longer what 0.14 replaced, so they are not put back.
        let t = Tmp::new("stale");
        let dir = t.install();
        setup(&t, "0.12 game", "0.13 game");
        put_in_place(&t.release(), &dir, "v0.13.0").unwrap();
        Tmp::put(&dir, &[(&game(), "0.14 game from the installer")]);
        assert_eq!(previous_version(&dir, "0.14.0"), None);
        assert_eq!(begin(&dir, "0.14.0"), Begun::Nothing);
        assert_eq!(begin(&dir, "0.14.0"), Begun::Nothing);
        assert_eq!(t.read(&game()).as_deref(), Some("0.14 game from the installer"));
        assert!(!dir.join(swap::PREVIOUS).exists());
    }

    #[test]
    fn a_notice_is_shown_once() {
        let t = Tmp::new("notice");
        write_notice(&t.install(), "Rolled back from v0.13.0 to 0.12.0.");
        assert_eq!(take_notice(&t.install()).as_deref(), Some("Rolled back from v0.13.0 to 0.12.0."));
        assert_eq!(take_notice(&t.install()), None);
    }

    /// Against the real v0.11.0 release (network): the API sizes, the HTTPS
    /// download through GitHub's redirect, and the size limit. Run with
    /// `cargo test -p ferrite -- --ignored real_release`.
    #[test]
    #[ignore]
    fn real_release_downloads_are_checked() {
        let tag = "v0.11.0";
        let assets = release_assets(tag).unwrap();
        let name = format!("ferrite-{tag}-linux-x64.tar.gz");
        let asset = assets.iter().find(|a| a.name == name).unwrap();
        let t = Tmp::new("net");
        let out = t.install().join(&name);
        // A limit below the real size is refused.
        assert!(download(&download_url(tag, &name), &out, asset.size / 2).unwrap_err().contains("bigger"));
        assert!(!out.exists());
        download(&download_url(tag, &name), &out, asset.size).unwrap();
        let (sum, len) = verify::sha256_file(&out).unwrap();
        assert_eq!(len, asset.size);
        assert_eq!(verify::check_download(&out, asset.size, &sum), Ok(()));
        // v0.11.0 predates SHA256SUMS: installing it is refused.
        assert!(!assets.iter().any(|a| a.name == verify::SUMS));
        assert!(install(tag, &t.install()).unwrap_err().contains("SHA256SUMS"));
        // Plain HTTP is never fetched.
        assert!(download(&download_url(tag, &name).replace("https://", "http://"), &out, asset.size).is_err());
    }
}
