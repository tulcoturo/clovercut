// SPDX-License-Identifier: AGPL-3.0-or-later
//! Versioned facial detections in source time, including explicit empty detections.
use concat_core::face::Face;
use std::path::{Path, PathBuf};

/// Source detections per second. Interpolation is bounded to one adjacent interval.
pub const RATE: u32 = 20;
/// Versioned sidecar directory independent of person/object masks.
pub fn directory(project: &Path, media: &str) -> PathBuf {
    crate::mask_dir(project, media, concat_project::model::Subject::Person).join("faces-yunet-v1")
}
/// An immutable-once-written sequence of facial detections.
pub struct Store {
    dir: PathBuf,
}
impl Store {
    /// Open a store without creating anything.
    pub fn open(dir: &Path) -> Self {
        Self { dir: dir.into() }
    }
    /// Whether a complete result (including no face) exists.
    pub fn contains(&self, millis: u64) -> bool {
        self.read(millis).is_some()
    }
    fn path(&self, millis: u64) -> PathBuf {
        self.dir.join(format!("{millis}.face"))
    }
    /// Read and validate a small bounded detection file.
    pub fn read(&self, millis: u64) -> Option<Vec<Face>> {
        use std::io::Read;
        let file = std::fs::File::open(self.path(millis)).ok()?;
        let mut data = Vec::new();
        file.take(249).read_to_end(&mut data).ok()?;
        if data.get(..4)? != b"FAC1" {
            return None;
        }
        let count = u32::from_le_bytes(data.get(4..8)?.try_into().ok()?) as usize;
        if count > 4 || data.len() != 8 + count * 60 {
            return None;
        }
        let mut faces = Vec::new();
        for row in data[8..].chunks_exact(60) {
            let values: Vec<f32> = row
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four bytes")))
                .collect();
            let face = Face {
                bounds: values[..4].try_into().ok()?,
                points: std::array::from_fn(|i| [values[4 + i * 2], values[5 + i * 2]]),
                confidence: values[14],
            };
            if !face.valid() {
                return None;
            }
            faces.push(face);
        }
        Some(faces)
    }
    /// Atomically commit one detection result. Cancelled work never leaves a partial result.
    pub fn put(&self, millis: u64, faces: &[Face]) -> Result<(), String> {
        if faces.len() > 4 || faces.iter().any(|face| !face.valid()) {
            return Err("face: invalid detections".into());
        }
        let mut bytes = b"FAC1".to_vec();
        bytes.extend((faces.len() as u32).to_le_bytes());
        for face in faces {
            for value in face
                .bounds
                .iter()
                .chain(face.points.iter().flatten())
                .chain(std::iter::once(&face.confidence))
            {
                bytes.extend(value.to_le_bytes());
            }
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let part = self
            .dir
            .join(format!("{millis}.{}.part", std::process::id()));
        std::fs::write(&part, bytes)
            .and_then(|()| std::fs::rename(&part, self.path(millis)))
            .map_err(|e| e.to_string())
    }
    /// Interpolate only neighbouring, spatially matched detections. Missing faces disappear.
    pub fn at(&self, seconds: f64, still: bool) -> Vec<Face> {
        if !seconds.is_finite() || seconds < 0.0 {
            return vec![];
        }
        if still {
            return self.read(0).unwrap_or_default();
        }
        let step = 1000 / u64::from(RATE);
        let time = seconds * 1000.0;
        let left = (time as u64 / step) * step;
        let t = ((time - left as f64) / step as f64) as f32;
        let first = self.read(left).unwrap_or_default();
        if t < 0.001 {
            return first;
        }
        let Some(second) = self.read(left + step) else {
            return if t < 0.5 { first } else { vec![] };
        };
        let mut used = vec![false; second.len()];
        let mut answer = Vec::new();
        for a in first {
            let nearest = second
                .iter()
                .enumerate()
                .filter(|(i, _)| !used[*i])
                .map(|(i, b)| {
                    (
                        i,
                        (a.points[2][0] - b.points[2][0]).hypot(a.points[2][1] - b.points[2][1]),
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((index, distance)) = nearest else {
                continue;
            };
            if distance > 0.15 {
                continue;
            }
            let b = &second[index];
            // Reject discontinuous scale changes instead of drawing a sticker across a cut.
            if (a.bounds[2] / b.bounds[2]).max(b.bounds[2] / a.bounds[2]) > 1.6 {
                continue;
            }
            used[index] = true;
            let blend = |x: f32, y: f32| x + (y - x) * t;
            let face = Face {
                bounds: std::array::from_fn(|i| blend(a.bounds[i], b.bounds[i])),
                points: std::array::from_fn(|i| {
                    std::array::from_fn(|j| blend(a.points[i][j], b.points[i][j]))
                }),
                confidence: a.confidence.min(b.confidence),
            };
            if face.valid() {
                answer.push(face);
            }
        }
        answer
    }
    /// Unanalysed source instants in the requested ranges, bounded to 24 hours per run.
    pub fn missing(&self, ranges: &[(f64, f64)], still: bool) -> Vec<u64> {
        if still {
            return if self.contains(0) { vec![] } else { vec![0] };
        }
        // One directory scan, rather than opening every cached instant on each UI edit.
        // Atomic writers validate contents before publishing a result.
        let existing: std::collections::BTreeSet<u64> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                let millis = name.strip_suffix(".face")?.parse().ok()?;
                let size = entry.metadata().ok()?.len();
                ([8, 68, 128, 188, 248].contains(&size)).then_some(millis)
            })
            .collect();
        let mut wanted = std::collections::BTreeSet::new();
        let step = 1000 / u64::from(RATE);
        for &(start, end) in ranges {
            if !start.is_finite() || !end.is_finite() || end < start {
                continue;
            }
            let from = (start.max(0.0) * f64::from(RATE)).floor() as u64;
            let to = ((end.max(0.0) * f64::from(RATE)).ceil() as u64).saturating_sub(1);
            for index in from..=to.min(from + 86400 * u64::from(RATE)) {
                let millis = index * step;
                if !existing.contains(&millis) {
                    wanted.insert(millis);
                }
            }
        }
        wanted.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation_matches_motion_and_never_bridges_missing_faces_or_large_cuts() {
        let dir =
            std::env::temp_dir().join(format!("clovercut-face-motion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir);
        let a = Face {
            bounds: [0.2, 0.2, 0.4, 0.5],
            points: [[0.3, 0.4], [0.5, 0.4], [0.4, 0.5], [0.35, 0.6], [0.45, 0.6]],
            confidence: 0.9,
        };
        let mut b = a.clone();
        b.bounds[0] += 0.1;
        for point in &mut b.points {
            point[0] += 0.1;
        }
        store.put(0, std::slice::from_ref(&a)).unwrap();
        store.put(50, std::slice::from_ref(&b)).unwrap();
        let between = store.at(0.025, false);
        assert_eq!(between.len(), 1);
        assert!((between[0].points[2][0] - 0.45).abs() < 0.0001);
        store.put(50, &[]).unwrap();
        assert!(store.at(0.025, false).is_empty());
        for point in &mut b.points {
            point[0] += 0.4;
        }
        store.put(50, &[b]).unwrap();
        assert!(store.at(0.025, false).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_empty_results_do_not_repeat_analysis_and_corrupt_cache_is_rejected() {
        let dir = std::env::temp_dir().join(format!("clovercut-face-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir);
        assert_eq!(store.missing(&[(0.0, 0.0)], true), [0]);
        store.put(0, &[]).unwrap();
        assert!(store.missing(&[(0.0, 0.0)], true).is_empty());
        assert!(store.at(1.0, true).is_empty());
        std::fs::write(store.path(0), b"FAC1xxxxxxxx").unwrap();
        assert!(!store.contains(0));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
