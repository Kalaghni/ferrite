//! The maps folder: where map documents (`.fmap`) live, so the map maker,
//! the launcher and tools can all list them.
//!
//! It is `maps/` next to the game, or the folder `FERRITE_MAPS` names. A map
//! is one `.fmap` file in it; unsaved work sits beside it as
//! `.fmap.autosave`. Files elsewhere still open by path.

use super::doc::{MapDoc, Symmetry};
use super::{tunnels, World, MAPS};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The extension of a map document.
pub const EXT: &str = "fmap";
/// Most maps listed, and the largest file read for a listing.
const MAX_FILES: usize = 500;
const MAX_LEN: u64 = super::doc::MAX_JSON as u64;
/// The seed new maps take a built-in map's ground from.
pub const SEED: u64 = 0xFE221;

/// The maps folder: `FERRITE_MAPS`, or `maps` beside the executable.
pub fn default_root() -> PathBuf {
    if let Some(d) = std::env::var_os("FERRITE_MAPS") {
        return d.into();
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("maps"))).unwrap_or_else(|| "maps".into())
}

/// Where unsaved work on `path` is kept.
pub fn autosave_of(path: &Path) -> PathBuf {
    path.with_extension("fmap.autosave")
}

fn modified(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).ok()?.modified().ok()
}

/// The autosave of `path`, if there is one newer than the file (or no file).
pub fn newer_autosave(path: &Path) -> Option<SystemTime> {
    let a = modified(&autosave_of(path))?;
    match modified(path) {
        Some(f) if f >= a => None,
        _ => Some(a),
    }
}

/// How long ago `t` was, roughly: "just now", "5 min ago", "3 days ago".
pub fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        86_400..=1_209_599 => plural(s / 86_400, "day"),
        1_209_600..=5_183_999 => plural(s / 604_800, "week"),
        _ => plural(s / 2_592_000, "month"),
    }
}

/// A map in the folder, as a listing shows it.
#[derive(Clone, Debug)]
pub struct MapFile {
    pub path: PathBuf,
    pub modified: SystemTime,
    /// What is in it, or why it could not be read.
    pub doc: Result<Summary, String>,
    /// When there is unsaved work newer than the file.
    pub autosave: Option<SystemTime>,
}

/// The parts of a map a listing needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub name: String,
    pub blurb: String,
    pub revision: u32,
    pub blocks: usize,
    pub trees: usize,
    pub mirrored: bool,
    /// Objectives A to E and the bases, as built.
    pub points: [[f32; 2]; 5],
    pub bases: [[f32; 2]; 2],
}

impl Summary {
    pub fn of(d: &MapDoc) -> Summary {
        let (points, bases) = d.layout();
        Summary {
            name: d.meta.name.clone(),
            blurb: d.meta.blurb.clone(),
            revision: d.meta.revision,
            blocks: d.brushes.len(),
            trees: d.trees.len(),
            mirrored: d.symmetry == Symmetry::HalfTurn,
            points,
            bases,
        }
    }
}

impl MapFile {
    /// The name to show: the map's own, or the file's.
    pub fn title(&self) -> String {
        match &self.doc {
            Ok(s) => s.name.clone(),
            Err(_) => self.path.file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default(),
        }
    }
}

/// Read a map document from disk.
pub fn read(path: &Path) -> Result<MapDoc, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_LEN {
        return Err("too large to be a map".into());
    }
    MapDoc::from_json(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Write a map document, all at once (a crash never leaves half a file).
pub fn write(path: &Path, d: &MapDoc) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("fmap.tmp");
    std::fs::write(&tmp, d.to_json()).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| e.to_string())
}

/// The `.fmap` files directly in `root`.
pub fn files(root: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<PathBuf> =
        rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXT))).collect();
    out.sort();
    out.truncate(MAX_FILES);
    out
}

/// Every map in `root`, the most recently changed first. Reading them all
/// can take a moment for big maps: list off the frame loop.
pub fn scan(root: &Path) -> Vec<MapFile> {
    let mut out: Vec<MapFile> = files(root)
        .into_iter()
        .map(|path| MapFile {
            modified: modified(&path).unwrap_or(SystemTime::UNIX_EPOCH),
            doc: read(&path).map(|d| Summary::of(&d)),
            autosave: newer_autosave(&path),
            path,
        })
        .collect();
    out.sort_by_key(|m| std::cmp::Reverse(m.autosave.unwrap_or(m.modified).max(m.modified)));
    out
}

/// A name the map format accepts: plain characters, upper case, up to 32.
pub fn clean_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == ' ' { c.to_ascii_uppercase() } else { ' ' }).take(32).collect();
    n.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A file in `root` for a new map called `name`, not yet taken.
pub fn fresh_path(root: &Path, name: &str) -> PathBuf {
    let stem = super::library::slug(name);
    let stem = if stem == "prefab" { "untitled".to_string() } else { stem };
    let mut p = root.join(format!("{stem}.{EXT}"));
    let mut n = 2;
    while p.exists() || autosave_of(&p).exists() {
        p = root.join(format!("{stem}-{n}.{EXT}"));
        n += 1;
    }
    p
}

/// What `s` means as a map: a path to a file, or the name of a map in
/// `root` (its file name or the name inside it, ignoring case). `None` when
/// it names nothing there.
pub fn resolve(root: &Path, s: &str) -> Option<PathBuf> {
    let p = PathBuf::from(s);
    let pathlike = s.contains(['/', '\\']) || p.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXT));
    if pathlike || p.is_file() {
        return Some(p);
    }
    let want = clean_name(s);
    let slug = super::library::slug(s);
    let all = files(root);
    if let Some(f) = all.iter().find(|f| f.file_stem().is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(&slug))) {
        return Some(f.clone());
    }
    all.into_iter().find(|f| read(f).is_ok_and(|d| clean_name(&d.meta.name) == want))
}

/// What a new map starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Flat ground, nothing on it.
    Blank,
    /// A built-in map's ground, sky and light, nothing on it.
    Ground(u8),
    /// A built-in map with everything on it, to remix.
    Copy(u8),
}

/// Whether built-in map `id` can be copied into a document: maps with
/// tunnels cannot be written out yet.
pub fn copyable(id: u8) -> bool {
    (id as usize) < MAPS.len() && tunnels::layout(id, |_, _| 0.0).is_empty()
}

/// A new map called `name`. A copy keeps only the west half where the map
/// is the same under a half turn, so an edit changes both sides.
pub fn starter(name: &str, from: Start) -> Result<MapDoc, String> {
    let name = clean_name(name);
    let name = if name.is_empty() { "UNTITLED".to_string() } else { name };
    let known = |id: u8| if (id as usize) < MAPS.len() { Ok(id) } else { Err(format!("there is no built-in map {id}")) };
    let mut d = match from {
        Start::Blank => MapDoc::blank(&name),
        Start::Ground(id) => {
            let id = known(id)?;
            let mut d = MapDoc::blank(&name);
            d.terrain.preset = Some(id);
            d.terrain.seed = (SEED ^ (SEED >> 32)) as u32;
            d.look.mood = id;
            d.look.ground = MAPS[id as usize].ground;
            d
        }
        Start::Copy(id) => {
            let id = known(id)?;
            let mut d = MapDoc::from_world(&World::with_map(SEED, id))?;
            d.fold();
            d
        }
    };
    d.meta.name = name;
    d.meta.revision = 0;
    d.validate()?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferrite-maps-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn maps_are_listed_named_and_found() {
        let root = scratch("list");
        let a = fresh_path(&root, "Twin Yards!");
        assert_eq!(a.file_name().unwrap(), "twin-yards.fmap");
        write(&a, &starter("twin yards!", Start::Blank).unwrap()).unwrap();
        // The next with that name gets its own file.
        let b = fresh_path(&root, "twin yards");
        assert_eq!(b.file_name().unwrap(), "twin-yards-2.fmap");
        write(&b, &starter("Dust Bowl", Start::Ground(3)).unwrap()).unwrap();
        std::fs::write(root.join("broken.fmap"), "{ not a map").unwrap();
        std::fs::write(root.join("notes.txt"), "not a map either").unwrap();

        let list = scan(&root);
        assert_eq!(list.len(), 3);
        let names: Vec<String> = list.iter().map(|m| m.title()).collect();
        assert!(names.contains(&"TWIN YARDS".to_string()) && names.contains(&"DUST BOWL".to_string()), "{names:?}");
        assert!(list.iter().any(|m| m.doc.is_err() && m.title() == "BROKEN"));

        // By file name, by the name inside, or by path.
        assert_eq!(resolve(&root, "twin-yards"), Some(a.clone()));
        assert_eq!(resolve(&root, "dust bowl"), Some(b.clone()));
        assert_eq!(resolve(&root, "nowhere"), None);
        assert_eq!(resolve(&root, "elsewhere/x.fmap"), Some(PathBuf::from("elsewhere/x.fmap")));

        // Unsaved work newer than the file is noticed.
        assert!(newer_autosave(&a).is_none());
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(autosave_of(&a), "{}").unwrap();
        assert!(newer_autosave(&a).is_some());
        assert!(scan(&root).iter().find(|m| m.path == a).unwrap().autosave.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn new_maps_start_blank_on_ground_or_as_copies() {
        let blank = starter("  ", Start::Blank).unwrap();
        assert_eq!(blank.meta.name, "UNTITLED");
        let ground = starter("hill fight", Start::Ground(2)).unwrap();
        assert_eq!((ground.terrain.preset, ground.look.mood, ground.brushes.len()), (Some(2), 2, 0));
        for id in 0..MAPS.len() as u8 {
            match starter("remix", Start::Copy(id)) {
                Ok(d) => {
                    assert!(copyable(id), "{id}");
                    assert!(!d.brushes.is_empty());
                    assert_eq!(d.meta.name, "REMIX");
                }
                Err(_) => assert!(!copyable(id), "{id} should copy"),
            }
        }
        assert!(starter("x", Start::Copy(99)).is_err());
    }
}
