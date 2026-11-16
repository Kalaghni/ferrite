//! Putting a release in place of the installed files, keeping the files it
//! replaces, and putting those back.
//!
//! Everything lives beside the executable, so every move is a rename on
//! one volume:
//!
//! - `ferrite-staging/` - the new files, copied in from the download.
//! - `ferrite-previous/` - the files the update replaced, plus `update.txt`,
//!   the record of what was done (see [`Journal`]).
//! - `ferrite-trash/` - files that could not be deleted yet (on Windows a
//!   running program can be renamed but not deleted). Emptied at each start.
//!
//! A swap moves each installed file into `ferrite-previous/`, then the new one
//! into its place. The record is written before the first move, so if the
//! swap fails - or the game is killed half way - [`restore`] can put every
//! file back from what it says, however far the swap got.

use std::path::{Path, PathBuf};

pub const STAGING: &str = "ferrite-staging";
pub const PREVIOUS: &str = "ferrite-previous";
pub const TRASH: &str = "ferrite-trash";
/// The record, inside `ferrite-previous/`.
pub const RECORD: &str = "update.txt";

/// Files a player may have edited: kept if they are already there.
pub const KEEP: [&str; 2] = ["start-server.bat", "start-server.sh"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Files are being moved. Found at a start, the swap was cut short.
    Swapping,
    /// In place, but the new version has not yet shown that it starts.
    Pending,
    /// The new version has started properly at least once.
    Confirmed,
}

/// What an update did, kept in `ferrite-previous/update.txt`.
#[derive(Clone, Debug, PartialEq)]
pub struct Journal {
    /// The version that was replaced (this build's own, e.g. "0.12.0").
    pub from: String,
    /// The release put in its place (e.g. "v0.13.0").
    pub to: String,
    pub status: Status,
    /// Starts of the new version since it was installed.
    pub launches: u32,
    /// Files that were moved into `ferrite-previous/`.
    pub replaced: Vec<String>,
    /// Files the update added that were not there before.
    pub added: Vec<String>,
}

/// A bare file name: no folders, nothing that climbs out of one.
fn plain(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0', '\n', '\r'])
}

impl Journal {
    pub fn to_text(&self) -> String {
        let status = match self.status {
            Status::Swapping => "swapping",
            Status::Pending => "pending",
            Status::Confirmed => "confirmed",
        };
        let mut s = format!("from={}\nto={}\nstatus={status}\nlaunches={}\n", self.from, self.to, self.launches);
        for f in &self.replaced {
            s += &format!("replaced={f}\n");
        }
        for f in &self.added {
            s += &format!("added={f}\n");
        }
        s
    }

    pub fn from_text(text: &str) -> Option<Journal> {
        let mut j = Journal {
            from: String::new(),
            to: String::new(),
            status: Status::Swapping,
            launches: 0,
            replaced: Vec::new(),
            added: Vec::new(),
        };
        let mut status = None;
        for line in text.lines() {
            let (k, v) = line.trim_end_matches('\r').split_once('=')?;
            match k {
                "from" => j.from = v.to_string(),
                "to" => j.to = v.to_string(),
                "status" => {
                    status = Some(match v {
                        "swapping" => Status::Swapping,
                        "pending" => Status::Pending,
                        "confirmed" => Status::Confirmed,
                        _ => return None,
                    })
                }
                "launches" => j.launches = v.parse().ok()?,
                "replaced" if plain(v) => j.replaced.push(v.to_string()),
                "added" if plain(v) => j.added.push(v.to_string()),
                _ => return None,
            }
        }
        j.status = status?;
        (!j.from.is_empty() && !j.to.is_empty()).then_some(j)
    }

    /// The record of the update in `dir`, if there is a readable one.
    pub fn load(dir: &Path) -> Option<Journal> {
        Journal::from_text(&std::fs::read_to_string(dir.join(PREVIOUS).join(RECORD)).ok()?)
    }

    /// Write the record: to a new file first, then renamed over the old one,
    /// so it is never found half written.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let prev = dir.join(PREVIOUS);
        let tmp = prev.join(format!("{RECORD}.new"));
        std::fs::write(&tmp, self.to_text()).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, prev.join(RECORD)).map_err(|e| format!("cannot write the update record: {e}"))
    }
}

/// Is a file the one the game starts from? It is moved last, so a swap that
/// fails early leaves the old game able to start.
fn is_game(name: &str) -> bool {
    name == "ferrite" || name == "ferrite.exe"
}

/// Get `path` out of the way: delete it, or, if that is refused (a running
/// program on Windows), rename it into `ferrite-trash/` to be deleted later.
pub fn discard(path: &Path, dir: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    let removed = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    if removed.is_ok() {
        return Ok(());
    }
    let trash = dir.join(TRASH);
    let _ = std::fs::create_dir_all(&trash);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let to = trash.join(format!("{stamp}-{}-{name}", std::process::id()));
    std::fs::rename(path, &to).map_err(|e| format!("cannot remove {}: {e}", path.display()))
}

/// Delete what earlier updates could not. Whatever is still in use stays.
pub fn empty_trash(dir: &Path) {
    let trash = dir.join(TRASH);
    if std::fs::remove_dir_all(&trash).is_ok() {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(&trash) {
        for e in rd.flatten() {
            let p = e.path();
            let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
    }
    let _ = std::fs::remove_dir(&trash);
}

/// Forget the kept previous version: its files are deleted (or trashed).
pub fn drop_previous(dir: &Path) -> Result<(), String> {
    discard(&dir.join(PREVIOUS), dir)
}

/// Put the files in `staged` in place of those in `dir`, keeping the ones
/// they replace in `dir/ferrite-previous/`. `from` and `to` are the two
/// versions, for the record. A previous version kept from an earlier update
/// must have been dropped first. If anything fails, the files already moved
/// are put back and nothing is left changed.
pub fn swap(staged: &Path, dir: &Path, from: &str, to: &str) -> Result<Journal, String> {
    swap_with(staged, dir, from, to, |a, b| std::fs::rename(a, b))
}

/// [`swap`], with the file moves done by `mv` (tests use it to fail one).
fn swap_with(
    staged: &Path,
    dir: &Path,
    from: &str,
    to: &str,
    mut mv: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<Journal, String> {
    let mut names = Vec::new();
    for e in std::fs::read_dir(staged).map_err(|e| e.to_string())?.flatten() {
        let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
        if e.path().is_file() && plain(&name) {
            names.push(name);
        }
    }
    names.sort_by_key(|n| (is_game(n), n.clone()));
    let mut j = Journal { from: from.into(), to: to.into(), status: Status::Swapping, launches: 0, replaced: vec![], added: vec![] };
    for name in &names {
        let exists = std::fs::symlink_metadata(dir.join(name)).is_ok();
        if exists && KEEP.contains(&name.as_str()) {
            continue;
        }
        if exists {
            j.replaced.push(name.clone());
        } else {
            j.added.push(name.clone());
        }
    }

    let prev = dir.join(PREVIOUS);
    if std::fs::symlink_metadata(&prev).is_ok() {
        return Err(format!("{} is still there from an earlier update", prev.display()));
    }
    std::fs::create_dir(&prev).map_err(|e| format!("cannot create {}: {e}", prev.display()))?;
    if let Err(e) = j.save(dir) {
        let _ = std::fs::remove_dir_all(&prev);
        return Err(e);
    }

    let moved = (|| {
        for name in names.iter().filter(|n| j.replaced.contains(n) || j.added.contains(n)) {
            let live = dir.join(name);
            if j.replaced.contains(name) {
                mv(&live, &prev.join(name)).map_err(|e| format!("cannot move {} aside: {e}", live.display()))?;
            }
            mv(&staged.join(name), &live).map_err(|e| format!("cannot put {} in place: {e}", live.display()))?;
        }
        j.status = Status::Pending;
        j.save(dir)
    })();
    match moved {
        Ok(()) => Ok(j),
        Err(e) => match restore(dir) {
            Ok(_) => Err(e),
            Err(r) => Err(format!("{e}; putting the old files back also failed: {r}")),
        },
    }
}

/// Put back the files the last update replaced, from what its record says,
/// and remove the files it added. It works however far the update got: a
/// file that was never moved is left alone. The new files, which may be
/// running, are renamed out of the way rather than deleted. On success the
/// kept previous version is used up and the record is gone; on failure the
/// record stays so the next start can try again.
pub fn restore(dir: &Path) -> Result<Journal, String> {
    let j = Journal::load(dir).ok_or("no previous version is kept")?;
    let prev = dir.join(PREVIOUS);
    let mut errors = Vec::new();
    for name in &j.added {
        if let Err(e) = discard(&dir.join(name), dir) {
            errors.push(e);
        }
    }
    // The game itself first: then it can start even if a later file fails.
    let mut replaced = j.replaced.clone();
    replaced.sort_by_key(|n| !is_game(n));
    for name in &replaced {
        let kept = prev.join(name);
        if std::fs::symlink_metadata(&kept).is_err() {
            continue; // never moved aside: the file in place is the old one
        }
        let live = dir.join(name);
        if let Err(e) = discard(&live, dir) {
            errors.push(e);
            continue;
        }
        if let Err(e) = std::fs::rename(&kept, &live) {
            errors.push(format!("cannot put back {}: {e}", live.display()));
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let _ = drop_previous(dir);
    Ok(j)
}

/// The record path, for messages.
pub fn record_path(dir: &Path) -> PathBuf {
    dir.join(PREVIOUS).join(RECORD)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Tmp {
            let p = std::env::temp_dir().join(format!("ferrite-swap-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(p.join("new")).unwrap();
            std::fs::create_dir_all(p.join("live")).unwrap();
            Tmp(p)
        }
        fn new_dir(&self) -> PathBuf {
            self.0.join("new")
        }
        fn live(&self) -> PathBuf {
            self.0.join("live")
        }
        fn read(&self, name: &str) -> Option<String> {
            std::fs::read_to_string(self.live().join(name)).ok()
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, files: &[(&str, &str)]) {
        for (n, t) in files {
            std::fs::write(dir.join(n), t).unwrap();
        }
    }

    const OLD: [(&str, &str); 4] = [
        ("ferrite.exe", "old game"),
        ("README.txt", "old readme"),
        ("start-server.bat", "my own options"),
        ("ferrite-client.txt", "name=me"),
    ];
    const NEW: [(&str, &str); 4] =
        [("ferrite.exe", "new game"), ("README.txt", "new readme"), ("start-server.bat", "new bat"), ("extra.txt", "extra")];

    #[test]
    fn journal_round_trips_and_rejects_junk() {
        let j = Journal {
            from: "0.12.0".into(),
            to: "v0.13.0".into(),
            status: Status::Pending,
            launches: 1,
            replaced: vec!["ferrite.exe".into(), "README.txt".into()],
            added: vec!["extra.txt".into()],
        };
        assert_eq!(Journal::from_text(&j.to_text()), Some(j.clone()));
        assert_eq!(Journal::from_text(&j.to_text().replace('\n', "\r\n")), Some(j));
        assert_eq!(Journal::from_text(""), None);
        assert_eq!(Journal::from_text("from=0.12.0\nto=v0.13.0\nstatus=bogus\n"), None);
        assert_eq!(Journal::from_text("from=0.12.0\nto=v0.13.0\n"), None); // no status
                                                                           // A record must never point outside the game's folder.
        assert_eq!(Journal::from_text("from=a\nto=b\nstatus=pending\nreplaced=../../evil.exe\n"), None);
        assert_eq!(Journal::from_text("from=a\nto=b\nstatus=pending\nadded=C:\\Windows\\x.dll\n"), None);
    }

    #[test]
    fn a_swap_keeps_the_old_files_and_a_restore_puts_them_back() {
        let t = Tmp::new("roundtrip");
        write(&t.new_dir(), &NEW);
        write(&t.live(), &OLD);
        let j = swap(&t.new_dir(), &t.live(), "0.12.0", "v0.13.0").unwrap();
        assert_eq!(j.status, Status::Pending);
        assert_eq!(j.replaced, ["README.txt", "ferrite.exe"]);
        assert_eq!(j.added, ["extra.txt"]);
        assert_eq!(Journal::load(&t.live()), Some(j));
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("new game"));
        assert_eq!(t.read("README.txt").as_deref(), Some("new readme"));
        assert_eq!(t.read("extra.txt").as_deref(), Some("extra"));
        assert_eq!(t.read("ferrite-previous/ferrite.exe").as_deref(), Some("old game"));
        // Edited by the player, and their settings: left as they were.
        assert_eq!(t.read("start-server.bat").as_deref(), Some("my own options"));
        assert_eq!(t.read("ferrite-client.txt").as_deref(), Some("name=me"));

        let back = restore(&t.live()).unwrap();
        assert_eq!(back.from, "0.12.0");
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"));
        assert_eq!(t.read("README.txt").as_deref(), Some("old readme"));
        assert_eq!(t.read("extra.txt"), None);
        assert_eq!(t.read("start-server.bat").as_deref(), Some("my own options"));
        assert_eq!(t.read("ferrite-client.txt").as_deref(), Some("name=me"));
        assert!(!t.live().join(PREVIOUS).exists(), "the kept version is used up");
        assert!(restore(&t.live()).is_err(), "nothing left to restore");
    }

    #[test]
    fn a_failed_swap_leaves_the_install_as_it_was() {
        // Fail each move in turn: wherever it stops, the install is the old one.
        for fail_at in 0..5 {
            let t = Tmp::new(&format!("fail{fail_at}"));
            write(&t.new_dir(), &NEW);
            write(&t.live(), &OLD);
            let mut n = 0;
            let r = swap_with(&t.new_dir(), &t.live(), "0.12.0", "v0.13.0", |a, b| {
                n += 1;
                if n > fail_at {
                    Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "disk said no"))
                } else {
                    std::fs::rename(a, b)
                }
            });
            let e = r.unwrap_err();
            assert!(e.contains("disk said no"), "{e}");
            assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"), "failing move {fail_at}");
            assert_eq!(t.read("README.txt").as_deref(), Some("old readme"), "failing move {fail_at}");
            assert_eq!(t.read("extra.txt"), None, "failing move {fail_at}");
            assert_eq!(t.read("start-server.bat").as_deref(), Some("my own options"));
            assert_eq!(t.read("ferrite-client.txt").as_deref(), Some("name=me"));
            assert!(!t.live().join(PREVIOUS).exists(), "failing move {fail_at}");
        }
    }

    #[test]
    fn a_swap_cut_short_is_undone_from_its_record() {
        // As if the game was killed after the README was swapped and the
        // game itself was moved aside, but before the new game was moved in.
        let t = Tmp::new("cut");
        write(&t.live(), &OLD);
        let live = t.live();
        let prev = live.join(PREVIOUS);
        std::fs::create_dir(&prev).unwrap();
        let j = Journal {
            from: "0.12.0".into(),
            to: "v0.13.0".into(),
            status: Status::Swapping,
            launches: 0,
            replaced: vec!["README.txt".into(), "ferrite.exe".into()],
            added: vec!["extra.txt".into()],
        };
        j.save(&live).unwrap();
        std::fs::rename(live.join("README.txt"), prev.join("README.txt")).unwrap();
        std::fs::write(live.join("README.txt"), "new readme").unwrap();
        std::fs::write(live.join("extra.txt"), "extra").unwrap();
        std::fs::rename(live.join("ferrite.exe"), prev.join("ferrite.exe")).unwrap();

        let back = restore(&live).unwrap();
        assert_eq!(back.status, Status::Swapping);
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"));
        assert_eq!(t.read("README.txt").as_deref(), Some("old readme"));
        assert_eq!(t.read("extra.txt"), None);
        assert!(!prev.exists());

        // Cut short before anything moved: nothing to put back, nothing lost.
        let prev = live.join(PREVIOUS);
        std::fs::create_dir(&prev).unwrap();
        j.save(&live).unwrap();
        restore(&live).unwrap();
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"));
        assert_eq!(t.read("README.txt").as_deref(), Some("old readme"));
    }

    #[test]
    fn a_swap_refuses_to_overwrite_a_kept_version() {
        let t = Tmp::new("kept");
        write(&t.new_dir(), &NEW);
        write(&t.live(), &OLD);
        std::fs::create_dir(t.live().join(PREVIOUS)).unwrap();
        assert!(swap(&t.new_dir(), &t.live(), "0.12.0", "v0.13.0").is_err());
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"));
        drop_previous(&t.live()).unwrap();
        assert!(swap(&t.new_dir(), &t.live(), "0.12.0", "v0.13.0").is_ok());
    }

    /// Windows will not overwrite or delete a running program, but renames
    /// it: an update and a rollback both work while the game is running.
    #[cfg(windows)]
    #[test]
    fn a_running_game_can_be_swapped_out_and_back() {
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let ping = Path::new(&sys).join("System32").join("PING.EXE");
        let t = Tmp::new("running");
        let (new, live) = (t.new_dir(), t.live());
        std::fs::copy(&ping, live.join("ferrite.exe")).unwrap();
        write(&new, &[("ferrite.exe", "new game")]);
        let run = |exe: PathBuf| {
            std::process::Command::new(exe).args(["-n", "30", "127.0.0.1"]).stdout(std::process::Stdio::null()).spawn().unwrap()
        };

        // Update while the old game runs, then roll back while it still runs.
        let mut old = run(live.join("ferrite.exe"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        swap(&new, &live, "0.12.0", "v0.13.0").unwrap();
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("new game"));
        restore(&live).unwrap();
        assert_eq!(std::fs::read(live.join("ferrite.exe")).unwrap(), std::fs::read(&ping).unwrap());
        let _ = old.kill();
        let _ = old.wait();

        // Update to a game that then runs, and roll back from it.
        std::fs::copy(&ping, new.join("ferrite.exe")).unwrap();
        write(&live, &[("ferrite.exe", "old game")]);
        swap(&new, &live, "0.12.0", "v0.13.0").unwrap();
        let mut fresh = run(live.join("ferrite.exe"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        restore(&live).unwrap();
        assert_eq!(t.read("ferrite.exe").as_deref(), Some("old game"));
        let _ = fresh.kill();
        let _ = fresh.wait();
        empty_trash(&live);
        assert!(!live.join(TRASH).exists());
    }

    #[test]
    fn files_in_use_go_to_the_trash_and_are_emptied_later() {
        let t = Tmp::new("trash");
        write(&t.live(), &[("a.txt", "a")]);
        discard(&t.live().join("a.txt"), &t.live()).unwrap();
        assert_eq!(t.read("a.txt"), None);
        discard(&t.live().join("missing.txt"), &t.live()).unwrap();
        std::fs::create_dir_all(t.live().join(TRASH)).unwrap();
        write(&t.live().join(TRASH), &[("1-ferrite.exe", "x")]);
        empty_trash(&t.live());
        assert!(!t.live().join(TRASH).exists());
    }
}
