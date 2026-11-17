// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Clovercut contributors
//! Public-domain reference recordings carried for Chatterbox, independent of
//! the optional Pocket bundle. See voices/README.md for readers and sources.
use std::path::{Path, PathBuf};

pub(crate) const RECORDINGS: &[(&str, &[u8])] = &[
    (
        "elizabeth-klett.flac",
        include_bytes!("../voices/elizabeth-klett.flac"),
    ),
    (
        "mark-f-smith.flac",
        include_bytes!("../voices/mark-f-smith.flac"),
    ),
    (
        "ruth-golding.flac",
        include_bytes!("../voices/ruth-golding.flac"),
    ),
    (
        "bob-neufeld.flac",
        include_bytes!("../voices/bob-neufeld.flac"),
    ),
];

pub(crate) fn carries(file: &str) -> bool {
    RECORDINGS.iter().any(|(name, _)| *name == file)
}

/// Materialize only these app-owned references. Repair partial or corrupted
/// cache entries, including a wrong recording with the same file size.
pub(crate) fn ensure(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join("stock-voices-v1");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    for (name, bytes) in RECORDINGS {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|stored| stored == *bytes) {
            continue;
        }
        std::fs::write(&path, bytes)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carried_references_decode_and_a_same_size_corrupt_cache_is_repaired() {
        let root =
            std::env::temp_dir().join(format!("clovercut-stock-voices-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let dir = ensure(&root).unwrap();
        for (name, bytes) in RECORDINGS {
            assert!(carries(name));
            let mut decoder = concat_media::AudioDecoder::open(
                dir.join(name),
                &concat_media::AudioOptions {
                    rate: 24000,
                    channels: 1,
                    format: concat_media::samples::SampleFormat::F32,
                    ..Default::default()
                },
            )
            .unwrap();
            let samples = decoder.collect_f32().unwrap();
            assert_eq!(samples.len(), 240000, "ten seconds: {name}");
            assert!(samples.iter().any(|sample| sample.abs() > 0.01));
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), *bytes);
        }
        let (name, bytes) = RECORDINGS[0];
        std::fs::write(dir.join(name), vec![0; bytes.len()]).unwrap();
        assert_eq!(ensure(&root).unwrap(), dir);
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}
