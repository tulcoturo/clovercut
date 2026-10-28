// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Filesystem search for missing media, independent of the graphical dialog.
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Owns one search; replacing or dropping it cancels its worker and replies.
#[derive(Debug, Default)]
pub struct Job {
    cancel: Arc<AtomicBool>,
}
impl Job {
    /// The cancellation token shared with this job's worker and reply.
    pub fn token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }
    /// Whether a reply belongs to this still-active request.
    pub fn accepts(&self, token: &Arc<AtomicBool>) -> bool {
        Arc::ptr_eq(&self.cancel, token) && !token.load(Ordering::Relaxed)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Find regular-file targets by exact basename, nearest to the selected root.
/// Equal-depth matches use folder-name order. Directory aliases are visited once;
/// valid file/folder symlinks remain usable, while broken links are ignored.
pub fn find_named_files(
    folder: &Path,
    wanted: &HashSet<String>,
    cancel: &AtomicBool,
) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    if wanted.is_empty() || cancel.load(Ordering::Relaxed) {
        return index;
    }
    let Ok(root) = std::fs::canonicalize(folder) else {
        return index;
    };
    let mut visited = HashSet::from([root]);
    let mut queue = VecDeque::from([folder.to_owned()]);
    while let Some(dir) = queue.pop_front() {
        if cancel.load(Ordering::Relaxed) {
            return HashMap::new();
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries
            .take_while(|_| !cancel.load(Ordering::Relaxed))
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return HashMap::new();
            }
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

/// Keep only still-missing snapshot items with an unchanged path and a live target.
/// A restored or independently relinked file must not be overwritten by a reply.
pub fn replacements(
    items: &[(String, String)],
    missing: &HashMap<String, String>,
    found: &HashMap<String, PathBuf>,
) -> Vec<(String, String)> {
    items
        .iter()
        .filter_map(|(id, old_path)| {
            if missing.get(id) != Some(old_path) {
                return None;
            }
            let name = Path::new(old_path).file_name()?.to_str()?;
            let target = found.get(name)?;
            target
                .is_file()
                .then(|| (id.clone(), target.to_string_lossy().into_owned()))
        })
        .collect()
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
    fn find(folder: &Path, names: &HashSet<String>) -> HashMap<String, PathBuf> {
        find_named_files(folder, names, &AtomicBool::new(false))
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
            let found = find(&root.0, &wanted(&["clip.mp4"]));
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
            let found = find(&root.0, &wanted(&["clip.mp4"]));
            assert_eq!(found.get("clip.mp4"), Some(&root.0.join("a/clip.mp4")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn broken_links_are_not_replacement_media() {
        use std::os::unix::fs::symlink;
        let root = Scratch::new();
        symlink(root.0.join("missing"), root.0.join("ghost.mp4")).unwrap();
        let found = find(&root.0, &wanted(&["ghost.mp4"]));
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
        let found = find(&root.0, &wanted(&["clip.mp4", "alias.mp4", "absent.mp4"]));
        assert_eq!(found.len(), 2);
        assert!(found["clip.mp4"].is_file());
        assert_eq!(found["alias.mp4"], root.0.join("alias.mp4"));
    }

    #[test]
    fn missing_roots_and_empty_requests_have_no_candidates() {
        let root = Scratch::new();
        root.file("clip.mp4");
        assert!(find(&root.0, &HashSet::new()).is_empty());
        assert!(find(&root.0.join("absent"), &wanted(&["clip.mp4"])).is_empty());
    }
    #[test]
    fn cancelled_searches_do_not_return_replacements() {
        let root = Scratch::new();
        root.file("clip.mp4");
        assert!(
            find_named_files(&root.0, &wanted(&["clip.mp4"]), &AtomicBool::new(true)).is_empty()
        );
    }

    #[test]
    fn replacing_or_dropping_a_job_cancels_and_rejects_its_reply() {
        let old = Job::default();
        let old_token = old.token();
        assert!(old.accepts(&old_token));
        let current = Job::default();
        assert!(!current.accepts(&old_token));
        drop(old);
        assert!(old_token.load(Ordering::Relaxed));
        assert!(!current.accepts(&old_token));
        let token = current.token();
        assert!(current.accepts(&token));
        drop(current);
        assert!(token.load(Ordering::Relaxed));
    }
    #[test]
    fn delayed_replies_preserve_restored_and_independently_changed_paths() {
        let root = Scratch::new();
        let target = root.file("clip.mp4");
        let items = ["restored", "changed", "missing"]
            .map(|id| (id.to_owned(), "/old/clip.mp4".to_owned()));
        let missing = HashMap::from([
            ("changed".to_owned(), "/elsewhere/clip.mp4".to_owned()),
            ("missing".to_owned(), "/old/clip.mp4".to_owned()),
        ]);
        let found = HashMap::from([("clip.mp4".to_owned(), target.clone())]);
        assert_eq!(
            replacements(&items, &missing, &found),
            vec![("missing".to_owned(), target.to_string_lossy().into_owned())]
        );
        std::fs::remove_file(target).unwrap();
        assert!(
            replacements(&items, &missing, &found).is_empty(),
            "vanished candidates cannot count as repairs"
        );
    }
}
