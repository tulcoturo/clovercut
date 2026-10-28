// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Filesystem search for missing media, independent of the graphical dialog.
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Find regular-file targets by exact basename, nearest to the selected root.
/// Equal-depth matches use folder-name order. Directory aliases are visited once;
/// valid file/folder symlinks remain usable, while broken links are ignored.
pub fn find_named_files(folder: &Path, wanted: &HashSet<String>) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    if wanted.is_empty() {
        return index;
    }
    let Ok(root) = std::fs::canonicalize(folder) else {
        return index;
    };
    let mut visited = HashSet::from([root]);
    let mut queue = VecDeque::from([folder.to_owned()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let kind = if kind.is_symlink() {
                let Ok(metadata) = std::fs::metadata(&path) else {
                    continue;
                };
                metadata.file_type()
            } else {
                kind
            };
            if kind.is_dir() {
                if let Ok(key) = std::fs::canonicalize(&path)
                    && visited.insert(key)
                {
                    queue.push_back(path);
                }
            } else if kind.is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
                && wanted.contains(name)
            {
                index.entry(name.to_owned()).or_insert(path);
                if index.len() == wanted.len() {
                    return index;
                }
            }
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "clovercut-relink-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn file(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture").unwrap();
            path
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn wanted(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn the_closest_duplicate_wins_in_either_sibling_folder() {
        for (shallow, deep) in [("a", "z"), ("z", "a")] {
            let root = Scratch::new();
            std::fs::create_dir(root.0.join("a")).unwrap();
            std::fs::create_dir(root.0.join("z")).unwrap();
            let expected = root.file(&format!("{shallow}/clip.mp4"));
            root.file(&format!("{deep}/nested/deeper/clip.mp4"));
            root.file("unrequested.mov");
            let found = find_named_files(&root.0, &wanted(&["clip.mp4"]));
            assert_eq!(found.get("clip.mp4"), Some(&expected));
            assert_eq!(found.len(), 1);
        }
    }

    #[test]
    fn equal_depth_matches_have_a_stable_name_order() {
        for order in [["a", "z"], ["z", "a"]] {
            let root = Scratch::new();
            for dir in order {
                root.file(&format!("{dir}/clip.mp4"));
            }
            let found = find_named_files(&root.0, &wanted(&["clip.mp4"]));
            assert_eq!(found.get("clip.mp4"), Some(&root.0.join("a/clip.mp4")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn broken_links_are_not_replacement_media() {
        use std::os::unix::fs::symlink;
        let root = Scratch::new();
        symlink(root.0.join("missing"), root.0.join("ghost.mp4")).unwrap();
        let found = find_named_files(&root.0, &wanted(&["ghost.mp4"]));
        assert!(
            found.is_empty(),
            "only regular-file targets can replace media"
        );
    }
    #[cfg(unix)]
    #[test]
    fn directory_cycles_terminate_and_valid_links_remain_usable() {
        use std::os::unix::fs::symlink;
        let root = Scratch::new();
        let source = root.file("real/clip.mp4");
        symlink(&root.0, root.0.join("real/back-to-root")).unwrap();
        symlink(root.0.join("real"), root.0.join("linked-folder")).unwrap();
        symlink(&source, root.0.join("alias.mp4")).unwrap();
        // The absent name forces traversal to finish instead of stopping early.
        let found = find_named_files(&root.0, &wanted(&["clip.mp4", "alias.mp4", "absent.mp4"]));
        assert_eq!(found.len(), 2);
        assert!(found["clip.mp4"].is_file());
        assert_eq!(found["alias.mp4"], root.0.join("alias.mp4"));
    }

    #[test]
    fn missing_roots_and_empty_requests_have_no_candidates() {
        let root = Scratch::new();
        root.file("clip.mp4");
        assert!(find_named_files(&root.0, &HashSet::new()).is_empty());
        assert!(find_named_files(&root.0.join("absent"), &wanted(&["clip.mp4"])).is_empty());
    }
}
