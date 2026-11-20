//! The asset library: prefabs in folders.
//!
//! It is a folder on disk (`prefabs/` next to the game, or wherever
//! `FERRITE_PREFABS` points) whose sub-folders are the library's folders
//! and whose `.fprefab` files are its models, plus a built-in set that
//! ships with the game: every block kind, and some cover, structures and
//! nature to start from. The map maker and its MCP server read the same
//! library, so a model an assistant saves shows up in the editor's grid.

use super::ops::{Prefab, PrefabBlock, PrefabTree};
use super::BlockKind;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Each block kind, its name and a starting size [x, y, z].
pub const BLOCKS: [(BlockKind, &str, [f32; 3]); 8] = [
    (BlockKind::Container, "CONTAINER", [6.0, 2.6, 2.5]),
    (BlockKind::Concrete, "CONCRETE", [6.0, 3.0, 0.6]),
    (BlockKind::Sandbag, "SANDBAGS", [4.0, 1.1, 0.9]),
    (BlockKind::Crate, "CRATE", [1.1, 1.1, 1.1]),
    (BlockKind::Metal, "STEEL", [3.0, 3.0, 3.0]),
    (BlockKind::Rock, "ROCK", [4.0, 2.5, 3.5]),
    (BlockKind::Step, "STEP", [1.2, 0.3, 1.2]),
    (BlockKind::Trunk, "POST", [0.5, 4.0, 0.5]),
];

/// Most files read from disk, and the deepest folder, so a huge or looping
/// folder cannot stall the editor.
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 4;
const MAX_FILE: u64 = 1 << 20;

pub struct Entry {
    /// "" for the top, else names like "COVER/WALLS".
    pub folder: String,
    pub prefab: Prefab,
    /// Where it was read from; `None` for a built-in one.
    pub file: Option<PathBuf>,
    /// Changes whenever the prefab does: for caching its picture.
    pub key: u64,
}

impl Entry {
    /// Footprint [x, z] and height.
    pub fn size(&self) -> [f32; 3] {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for b in &self.prefab.blocks {
            for a in 0..3 {
                lo[a] = lo[a].min(b.min[a]);
                hi[a] = hi[a].max(b.max[a]);
            }
        }
        for t in &self.prefab.trees {
            lo = [lo[0].min(t.at[0] - t.radius), lo[1].min(0.0), lo[2].min(t.at[1] - t.radius)];
            hi = [hi[0].max(t.at[0] + t.radius), hi[1].max(t.height), hi[2].max(t.at[1] + t.radius)];
        }
        if lo[0] > hi[0] {
            return [0.0; 3];
        }
        [hi[0] - lo[0], hi[2] - lo[2], hi[1]]
    }
}

pub struct Library {
    pub root: PathBuf,
    /// Every folder, built-in or on disk, even empty ones.
    pub folders: Vec<String>,
    pub entries: Vec<Entry>,
}

/// `FERRITE_PREFABS`, or `prefabs/` next to the running program.
pub fn default_root() -> PathBuf {
    if let Some(d) = std::env::var_os("FERRITE_PREFABS") {
        return d.into();
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("prefabs"))).unwrap_or_else(|| "prefabs".into())
}

/// A file name for a prefab name.
pub fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "prefab".into()
    } else {
        s
    }
}

/// A folder name as the library writes it: upper case, `/` between parts,
/// nothing that climbs out of the library.
pub fn clean_folder(f: &str) -> String {
    f.split(['/', '\\'])
        .map(|p| p.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_').collect::<String>())
        .filter(|p| !p.is_empty() && p != "." && p != "..")
        .map(|p| p.to_uppercase())
        .collect::<Vec<_>>()
        .join("/")
}

fn key_of(folder: &str, p: &Prefab) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    folder.hash(&mut h);
    serde_json::to_string(p).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// A fingerprint of the folder on disk: names, sizes and times. When it
/// changes, the library is read again.
pub fn signature(root: &Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut stack = vec![(root.to_path_buf(), 0)];
    let mut n = 0;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            n += 1;
            if n > MAX_FILES {
                return h.finish();
            }
            e.file_name().hash(&mut h);
            if let Ok(m) = e.metadata() {
                m.len().hash(&mut h);
                m.modified().ok().hash(&mut h);
                if m.is_dir() && depth < MAX_DEPTH {
                    stack.push((e.path(), depth + 1));
                }
            }
        }
    }
    h.finish()
}

fn b(kind: BlockKind, min: [f32; 3], max: [f32; 3], tint: u8) -> PrefabBlock {
    PrefabBlock { kind, min, max, tint }
}

fn pf(name: &str, about: &str, blocks: Vec<PrefabBlock>, trees: Vec<PrefabTree>) -> Prefab {
    Prefab { name: name.into(), about: about.into(), blocks, trees }
}

/// The models that ship with the game.
pub fn builtin() -> Vec<(String, Prefab)> {
    use BlockKind::*;
    let mut out: Vec<(String, Prefab)> = BLOCKS
        .iter()
        .map(|&(k, name, s)| {
            let blk = b(k, [-s[0] / 2.0, 0.0, -s[2] / 2.0], [s[0] / 2.0, s[1], s[2] / 2.0], 0);
            ("BLOCKS".to_string(), pf(name, "One block", vec![blk], Vec::new()))
        })
        .collect();
    let mut add = |folder: &str, p: Prefab| out.push((folder.to_string(), p));

    // Cover.
    add(
        "COVER",
        pf(
            "SANDBAG LINE",
            "Waist-high bags in a shallow arc",
            vec![
                b(Sandbag, [-6.0, 0.0, 0.3], [-2.0, 1.1, 1.2], 0),
                b(Sandbag, [-2.0, 0.0, 0.0], [2.0, 1.1, 0.9], 1),
                b(Sandbag, [2.0, 0.0, 0.3], [6.0, 1.1, 1.2], 0),
            ],
            Vec::new(),
        ),
    );
    add(
        "COVER",
        pf(
            "SANDBAG NEST",
            "A U of bags, open at the back",
            vec![
                b(Sandbag, [-2.5, 0.0, -2.0], [2.5, 1.2, -1.1], 0),
                b(Sandbag, [-2.5, 0.0, -1.1], [-1.6, 1.1, 1.8], 1),
                b(Sandbag, [1.6, 0.0, -1.1], [2.5, 1.1, 1.8], 1),
            ],
            Vec::new(),
        ),
    );
    add(
        "COVER",
        pf(
            "CRATE STACK",
            "Crates three, two and one high",
            vec![
                b(Crate, [-1.65, 0.0, -0.55], [-0.55, 1.1, 0.55], 0),
                b(Crate, [-0.55, 0.0, -0.55], [0.55, 1.1, 0.55], 1),
                b(Crate, [0.55, 0.0, -0.55], [1.65, 1.1, 0.55], 0),
                b(Crate, [-1.1, 1.1, -0.5], [0.0, 2.2, 0.6], 1),
                b(Crate, [0.0, 1.1, -0.6], [1.1, 2.2, 0.5], 0),
                b(Crate, [-0.5, 2.2, -0.55], [0.6, 3.3, 0.55], 1),
            ],
            Vec::new(),
        ),
    );
    add(
        "COVER",
        pf(
            "JERSEY BARRIERS",
            "Four low concrete barriers with gaps to shoot through",
            (0..4)
                .map(|i| {
                    let x = -5.4 + i as f32 * 2.7;
                    b(Concrete, [x, 0.0, -0.3], [x + 2.0, 0.85, 0.3], 0)
                })
                .collect(),
            Vec::new(),
        ),
    );
    add(
        "COVER",
        pf(
            "LOG PILE",
            "Felled trunks stacked three, two, one",
            vec![
                b(Trunk, [-2.0, 0.0, -0.8], [2.0, 0.5, -0.3], 0),
                b(Trunk, [-2.0, 0.0, -0.25], [2.0, 0.5, 0.25], 0),
                b(Trunk, [-2.0, 0.0, 0.3], [2.0, 0.5, 0.8], 0),
                b(Trunk, [-1.9, 0.5, -0.55], [1.9, 1.0, -0.05], 0),
                b(Trunk, [-1.9, 0.5, 0.05], [1.9, 1.0, 0.55], 0),
                b(Trunk, [-1.8, 1.0, -0.25], [1.8, 1.5, 0.25], 0),
            ],
            Vec::new(),
        ),
    );

    // Structures.
    add(
        "STRUCTURES",
        pf(
            "CONTAINER STACK",
            "Two containers with one across the top",
            vec![
                b(Container, [-3.0, 0.0, -2.6], [3.0, 2.6, -0.1], 0),
                b(Container, [-3.0, 0.0, 0.1], [3.0, 2.6, 2.6], 1),
                b(Container, [-1.25, 2.6, -3.0], [1.25, 5.2, 3.0], 2),
            ],
            Vec::new(),
        ),
    );
    add(
        "STRUCTURES",
        pf(
            "CONTAINER CORNER",
            "Two containers in an L",
            vec![b(Container, [-3.0, 0.0, -1.25], [3.0, 2.6, 1.25], 3), b(Container, [3.2, 0.0, -1.25], [5.7, 2.6, 4.75], 0)],
            Vec::new(),
        ),
    );
    {
        // Bunker: 8 x 6, a firing slit along the front, a door at the back.
        let (w, d, h, t) = (4.0, 3.0, 2.4, 0.6);
        add(
            "STRUCTURES",
            pf(
                "BUNKER",
                "Concrete box with a firing slit and a back door",
                vec![
                    b(Concrete, [-w, 0.0, -d], [w, 1.3, -d + t], 0),
                    b(Concrete, [-w, 1.9, -d], [w, h, -d + t], 0),
                    b(Concrete, [-w, 1.3, -d], [-w + t, 1.9, -d + t], 0),
                    b(Concrete, [w - t, 1.3, -d], [w, 1.9, -d + t], 0),
                    b(Concrete, [-w, 0.0, -d + t], [-w + t, h, d], 0),
                    b(Concrete, [w - t, 0.0, -d + t], [w, h, d], 0),
                    b(Concrete, [-w + t, 0.0, d - t], [-0.8, h, d], 0),
                    b(Concrete, [0.8, 0.0, d - t], [w - t, h, d], 0),
                    b(Concrete, [-0.8, 2.0, d - t], [0.8, h, d], 0),
                    b(Concrete, [-w - 0.3, h, -d - 0.3], [w + 0.3, h + 0.4, d + 0.3], 1),
                ],
                Vec::new(),
            ),
        );
    }
    add(
        "STRUCTURES",
        pf(
            "WALL WITH DOORWAY",
            "Two wall runs and a lintel over a 1.6 m door",
            vec![
                b(Concrete, [-3.8, 0.0, -0.3], [-0.8, 3.0, 0.3], 0),
                b(Concrete, [0.8, 0.0, -0.3], [3.8, 3.0, 0.3], 0),
                b(Concrete, [-0.8, 2.3, -0.3], [0.8, 3.0, 0.3], 0),
            ],
            Vec::new(),
        ),
    );
    {
        // Watchtower: posts, a steel deck at 3 m, a parapet, and steps up.
        let mut v = Vec::new();
        for (x, z) in [(-1.7, -1.7), (1.2, -1.7), (-1.7, 1.2), (1.2, 1.2)] {
            v.push(b(Trunk, [x, 0.0, z], [x + 0.5, 2.75, z + 0.5], 0));
        }
        v.push(b(Metal, [-1.7, 2.75, -1.7], [1.7, 3.0, 1.7], 0));
        v.push(b(Concrete, [1.4, 3.0, -1.7], [1.7, 4.0, 1.7], 0));
        v.push(b(Concrete, [-1.7, 3.0, 1.4], [1.4, 4.0, 1.7], 0));
        v.push(b(Concrete, [-1.7, 3.0, -1.7], [1.4, 4.0, -1.4], 0));
        for i in 0..6 {
            let x = -5.3 + i as f32 * 0.6;
            v.push(b(Step, [x, 0.0, -1.0], [x + 0.6, 0.45 * (i + 1) as f32, 0.2], 0));
        }
        add("STRUCTURES", pf("WATCHTOWER", "A steel deck on posts, with steps up and a parapet", v, Vec::new()));
    }
    add(
        "STRUCTURES",
        pf(
            "RUIN CORNER",
            "Broken walls in an L, with rubble",
            vec![
                b(Concrete, [-4.0, 0.0, -0.3], [-2.0, 3.2, 0.3], 0),
                b(Concrete, [-2.0, 0.0, -0.3], [0.0, 2.3, 0.3], 0),
                b(Concrete, [0.0, 0.0, -0.3], [2.0, 1.3, 0.3], 0),
                b(Concrete, [-4.0, 0.0, 0.3], [-3.4, 2.8, 2.3], 0),
                b(Concrete, [-4.0, 0.0, 2.3], [-3.4, 1.6, 4.3], 0),
                b(Rock, [0.6, 0.0, 1.0], [1.8, 0.6, 2.0], 0),
                b(Rock, [-1.8, 0.0, 1.6], [-0.9, 0.45, 2.4], 0),
            ],
            Vec::new(),
        ),
    );

    // Nature.
    add(
        "NATURE",
        pf(
            "BOULDERS",
            "Three rocks, big to small",
            vec![
                b(Rock, [-3.0, 0.0, -2.0], [1.0, 2.6, 1.5], 0),
                b(Rock, [1.2, 0.0, -1.0], [3.6, 1.5, 1.2], 1),
                b(Rock, [-1.0, 0.0, 1.8], [0.6, 0.9, 3.0], 2),
            ],
            Vec::new(),
        ),
    );
    add(
        "NATURE",
        pf(
            "TREE CLUMP",
            "Three trees round a rock",
            vec![b(Rock, [-0.8, 0.0, -0.6], [0.9, 1.0, 0.8], 0)],
            vec![
                PrefabTree { at: [-3.0, -1.5], height: 9.0, radius: 2.8, pine: false },
                PrefabTree { at: [2.8, -2.0], height: 7.0, radius: 2.2, pine: false },
                PrefabTree { at: [0.5, 3.0], height: 11.0, radius: 2.0, pine: true },
            ],
        ),
    );
    add(
        "NATURE",
        pf(
            "PINE STAND",
            "Five pines",
            Vec::new(),
            [(-4.0, -1.0, 12.0), (-1.0, -3.5, 10.0), (2.5, -1.5, 13.0), (0.0, 2.0, 9.0), (4.0, 2.5, 11.0)]
                .iter()
                .map(|&(x, z, h)| PrefabTree { at: [x, z], height: h, radius: 1.8, pine: true })
                .collect(),
        ),
    );
    out
}

impl Library {
    /// The built-in models and everything under `root`.
    pub fn load(root: &Path) -> Library {
        let mut entries: Vec<Entry> =
            builtin().into_iter().map(|(folder, prefab)| Entry { key: key_of(&folder, &prefab), folder, prefab, file: None }).collect();
        let mut folders: Vec<String> = Vec::new();
        let mut stack = vec![(root.to_path_buf(), String::new(), 0)];
        let mut files = 0;
        while let Some((dir, folder, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let path = e.path();
                let Ok(meta) = e.metadata() else { continue };
                if meta.is_dir() {
                    let name = clean_folder(&e.file_name().to_string_lossy());
                    if depth < MAX_DEPTH && !name.is_empty() {
                        let sub = if folder.is_empty() { name } else { format!("{folder}/{name}") };
                        folders.push(sub.clone());
                        stack.push((path, sub, depth + 1));
                    }
                } else if path.extension().is_some_and(|x| x == "fprefab") && meta.len() <= MAX_FILE {
                    files += 1;
                    if files > MAX_FILES {
                        break;
                    }
                    let Ok(text) = std::fs::read_to_string(&path) else { continue };
                    let Ok(mut prefab) = serde_json::from_str::<Prefab>(&text) else { continue };
                    if prefab.blocks.is_empty() && prefab.trees.is_empty() {
                        continue;
                    }
                    prefab.name = prefab.name.trim().to_uppercase();
                    if prefab.name.is_empty() {
                        prefab.name = slug(&e.file_name().to_string_lossy()).to_uppercase();
                    }
                    entries.push(Entry { key: key_of(&folder, &prefab), folder: folder.clone(), prefab, file: Some(path) });
                }
            }
        }
        for e in &entries {
            // Every level of a model's folder is a folder too.
            let mut f = e.folder.as_str();
            while !f.is_empty() {
                folders.push(f.to_string());
                f = f.rsplit_once('/').map(|(a, _)| a).unwrap_or("");
            }
        }
        folders.sort();
        folders.dedup();
        // Built-in folders first, in the order they were written.
        let order = ["BLOCKS", "COVER", "STRUCTURES", "NATURE"];
        folders.sort_by_key(|f| (order.iter().position(|o| f == o).unwrap_or(order.len()), f.clone()));
        Library { root: root.to_path_buf(), folders, entries }
    }

    /// The folders directly inside `folder`, and the models in it.
    pub fn children(&self, folder: &str) -> (Vec<String>, Vec<usize>) {
        let subs = self
            .folders
            .iter()
            .filter(|f| match f.rsplit_once('/') {
                Some((parent, _)) => parent == folder,
                None => folder.is_empty(),
            })
            .cloned()
            .collect();
        let items = self.entries.iter().enumerate().filter(|(_, e)| e.folder == folder).map(|(i, _)| i).collect();
        (subs, items)
    }

    /// Models in `folder` and everything under it.
    pub fn count(&self, folder: &str) -> usize {
        self.under(folder).count()
    }

    /// Models in `folder` and everything under it, in library order.
    pub fn under<'a>(&'a self, folder: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        let pre = format!("{folder}/");
        self.entries.iter().filter(move |e| folder.is_empty() || e.folder == folder || e.folder.starts_with(&pre))
    }

    /// A model by name, or by "FOLDER/NAME"; case does not matter. Models
    /// on disk win over built-in ones of the same name.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        let want = name.trim().to_uppercase();
        let (folder, leaf) = match want.rsplit_once('/') {
            Some((f, n)) => (Some(clean_folder(f)), n.trim().to_string()),
            None => (None, want),
        };
        let hit = |e: &&Entry| e.prefab.name == leaf && folder.as_ref().is_none_or(|f| *f == e.folder);
        self.entries.iter().rev().find(hit)
    }

    /// Write a model into `folder` (made if need be). Saving a name again
    /// replaces it.
    pub fn save(&self, folder: &str, prefab: &Prefab) -> Result<PathBuf, String> {
        let folder = clean_folder(folder);
        let mut dir = self.root.clone();
        for part in folder.split('/').filter(|p| !p.is_empty()) {
            dir.push(part.to_lowercase());
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        let file = dir.join(format!("{}.fprefab", slug(&prefab.name)));
        let text = serde_json::to_string_pretty(prefab).map_err(|e| e.to_string())?;
        std::fs::write(&file, text).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_models_are_sound() {
        let lib = Library::load(Path::new("no-such-folder-for-a-test"));
        assert!(lib.entries.len() >= 20);
        assert_eq!(&lib.folders[..4], &["BLOCKS", "COVER", "STRUCTURES", "NATURE"]);
        for e in &lib.entries {
            for b in &e.prefab.blocks {
                assert!((0..3).all(|a| b.max[a] - b.min[a] >= 0.1), "{}: a block has no size", e.prefab.name);
                assert!(b.min[1] >= 0.0, "{}: a block is under its ground", e.prefab.name);
            }
        }
        let (subs, items) = lib.children("");
        assert_eq!(subs.len(), 4);
        assert!(items.is_empty());
        assert_eq!(lib.count(""), lib.entries.len());
        assert_eq!(lib.find("bunker").unwrap().folder, "STRUCTURES");
        assert_eq!(lib.find("cover/sandbag nest").unwrap().prefab.name, "SANDBAG NEST");
        assert!(lib.find("blocks/sandbag nest").is_none());
    }

    #[test]
    fn folders_on_disk_join_the_library() {
        let root = std::env::temp_dir().join(format!("ferrite-lib-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let lib = Library::load(&root);
        let p = pf("Gate", "", vec![b(BlockKind::Concrete, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0)], Vec::new());
        lib.save("Cover/../Walls", &p).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        let lib = Library::load(&root);
        assert!(lib.folders.contains(&"COVER/WALLS".to_string()), "{:?}", lib.folders);
        assert!(lib.folders.contains(&"EMPTY".to_string()));
        assert_eq!(lib.find("gate").unwrap().folder, "COVER/WALLS");
        assert_eq!(lib.children("COVER").0, vec!["COVER/WALLS".to_string()]);
        assert_eq!(lib.count("COVER"), 6);
        let _ = std::fs::remove_dir_all(&root);
    }
}
