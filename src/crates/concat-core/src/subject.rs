// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! A cached subject matte supplied to person-aware shader passes.
use std::sync::Arc;

/// Built-in passes that require a subject matte, independently of background removal.
pub fn needs_subject(id: &str) -> bool {
    matches!(
        id,
        "concat.subject-neon"
            | "concat.subject-electric"
            | "concat.subject-cyber-aura"
            | "concat.subject-clone-trail"
    )
}

/// Effects tied to source geometry run before ordinary picture distortions.
pub fn source_effect(id: &str) -> bool {
    needs_subject(id) || crate::face::needs_face(id)
}

/// Maximum mask extent uploaded for subject effects, independent of delivery resolution.
pub const MAX_SIDE: u32 = 512;

/// Identity source mapping.
pub const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// An eight-bit matte, with a mapping from effect-picture UVs to matte UVs.
/// Only the small analysis mask is uploaded; inference never runs in a shader.
#[derive(Clone, Debug, PartialEq)]
pub struct SubjectMask {
    /// Content identity, independent of placement/crop mapping.
    pub id: u64,
    /// Mask pixels across.
    pub width: u32,
    /// Mask pixels down.
    pub height: u32,
    /// Row-major coverage, 0 background and 255 subject.
    pub gray: Arc<[u8]>,
    /// Effect UVs to the source the model analysed.
    pub mapping: [f32; 6],
}
impl SubjectMask {
    /// Validate and own an analysed matte. A missing mask is never an opaque frame.
    pub fn new(width: u32, height: u32, gray: Vec<u8>) -> Option<Self> {
        let size = (width as usize).checked_mul(height as usize)?;
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE || gray.len() != size
        {
            return None;
        }
        let mut id = 0xcbf29ce484222325u64;
        for byte in width
            .to_le_bytes()
            .into_iter()
            .chain(height.to_le_bytes())
            .chain(gray.iter().copied())
        {
            id ^= u64::from(byte);
            id = id.wrapping_mul(0x100000001b3);
        }
        Some(Self {
            id,
            width,
            height,
            gray: gray.into(),
            mapping: IDENTITY,
        })
    }
    /// No subject detected or no analysed frame yet: subject-aware passes are idle.
    pub fn empty() -> Self {
        Self::new(1, 1, vec![0]).expect("one pixel")
    }

    /// Compose a crop/flip/warp without copying or resampling the mask pixels.
    pub fn remapped(&self, next: [f32; 6]) -> Option<Self> {
        let [a, b, c, d, e, f] = self.mapping;
        let [g, h, i, j, k, l] = next;
        let mapping = [
            a * g + b * j,
            a * h + b * k,
            a * i + b * l + c,
            d * g + e * j,
            d * h + e * k,
            d * i + e * l + f,
        ];
        mapping.iter().all(|v| v.is_finite()).then(|| Self {
            mapping,
            ..self.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_sizes_and_nonfinite_maps_are_rejected() {
        assert!(SubjectMask::new(0, 2, vec![]).is_none());
        assert!(SubjectMask::new(2, 2, vec![0; 3]).is_none());
        assert!(SubjectMask::empty().remapped([f32::NAN; 6]).is_none());
    }
    #[test]
    fn identities_include_shape_and_pixels_but_maps_share_storage() {
        let mask = SubjectMask::new(2, 1, vec![0, 255]).unwrap();
        assert_ne!(mask.id, SubjectMask::new(1, 2, vec![0, 255]).unwrap().id);
        let flipped = mask.remapped([-1.0, 0.0, 1.0, 0.0, 1.0, 0.0]).unwrap();
        let twice = flipped.remapped([-1.0, 0.0, 1.0, 0.0, 1.0, 0.0]).unwrap();
        assert_eq!(twice.mapping, IDENTITY);
        assert!(Arc::ptr_eq(&mask.gray, &flipped.gray));
    }
}
