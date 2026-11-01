// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Content identity for reusing project-owned audio copies.
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Reuse only a regular owned file with identical bytes, never a directory/link.
/// Equal byte lengths alone do not establish that two recordings are the same.
pub fn matches_copy(source: &Path, target: &Path) -> io::Result<bool> {
    let existing = std::fs::symlink_metadata(target)?;
    let original = std::fs::metadata(source)?;
    if !existing.is_file() || !original.is_file() || existing.len() != original.len() {
        return Ok(false);
    }
    // Bounded 64 KiB buffers amortise reads on long recordings without loading
    // the complete files. The local repeated-read baseline is recorded in the
    // editor workflow audit; file length alone still never establishes identity.
    let mut source = BufReader::with_capacity(64 * 1024, std::fs::File::open(source)?);
    let mut target = BufReader::with_capacity(64 * 1024, std::fs::File::open(target)?);
    loop {
        let left = source.fill_buf()?;
        let right = target.fill_buf()?;
        if left.is_empty() || right.is_empty() {
            return Ok(left.is_empty() && right.is_empty());
        }
        let shared = left.len().min(right.len());
        if left[..shared] != right[..shared] {
            return Ok(false);
        }
        source.consume(shared);
        target.consume(shared);
    }
}

/// Choose a reusable identical copy or a free numbered name without clobbering.
/// A dangling link also occupies its name and must never become a copy target.
pub fn destination(source: &Path, folder: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut target = folder.join(format!("{stem}{extension}"));
    let mut next = 2;
    while target.symlink_metadata().is_ok() && !matches_copy(source, &target).unwrap_or(false) {
        target = folder.join(format!("{stem} {next}{extension}"));
        next += 1;
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "clovercut-audio-copy-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn same_size_recordings_with_different_bytes_are_not_the_same_copy() {
        let root = Scratch::new();
        let source = root.0.join("source.wav");
        let target = root.0.join("owned.wav");
        let mut bytes = vec![42; 100_000];
        std::fs::write(&source, &bytes).unwrap();
        std::fs::write(&target, &bytes).unwrap();
        assert!(matches_copy(&source, &target).unwrap());
        assert_eq!(destination(&source, &root.0, "owned", ".wav"), target);
        bytes[99_999] = 43;
        std::fs::write(&target, &bytes).unwrap();
        assert_eq!(
            std::fs::metadata(&source).unwrap().len(),
            std::fs::metadata(&target).unwrap().len()
        );
        assert!(!matches_copy(&source, &target).unwrap());
        assert_eq!(
            destination(&source, &root.0, "owned", ".wav"),
            root.0.join("owned 2.wav")
        );
        assert_eq!(std::fs::read(&source).unwrap()[99_999], 42);
        assert_eq!(std::fs::read(&target).unwrap()[99_999], 43);
    }
    #[cfg(unix)]
    #[test]
    fn links_are_not_project_owned_copies_even_with_identical_contents() {
        use std::os::unix::fs::symlink;
        let root = Scratch::new();
        let source = root.0.join("source.wav");
        let target = root.0.join("owned.wav");
        std::fs::write(&source, b"recording").unwrap();
        symlink(&source, &target).unwrap();
        assert!(!matches_copy(&source, &target).unwrap());
        assert_eq!(
            destination(&source, &root.0, "owned", ".wav"),
            root.0.join("owned 2.wav")
        );
        std::fs::remove_file(&target).unwrap();
        symlink(root.0.join("missing"), &target).unwrap();
        assert_eq!(
            destination(&source, &root.0, "owned", ".wav"),
            root.0.join("owned 2.wav")
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"recording");
    }
}
