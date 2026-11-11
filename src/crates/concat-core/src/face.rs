// SPDX-License-Identifier: AGPL-3.0-or-later
//! Source-space facial geometry shared by analysis and the GPU.

/// Whether an effect requires tracked facial landmarks.
pub fn needs_face(id: &str) -> bool {
    matches!(
        id,
        "concat.face-sunglasses" | "concat.face-dog" | "concat.big-head" | "concat.face-alien"
    )
}

/// Face position and five landmarks: right eye, left eye, nose, mouth corners.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// Source-space bounding rectangle, x/y/width/height.
    pub bounds: [f32; 4],
    /// Source-space landmarks, each x/y.
    pub points: [[f32; 2]; 5],
    /// Detection confidence.
    pub confidence: f32,
}

impl Face {
    /// Reject malformed model/cache values before they reach tracking or rendering.
    pub fn valid(&self) -> bool {
        self.bounds
            .iter()
            .chain(self.points.iter().flatten())
            .all(|v| v.is_finite() && v.abs() < 8.0)
            && self.bounds[2] > 0.0
            && self.bounds[3] > 0.0
            && self.confidence.is_finite()
            && (0.0..=1.0).contains(&self.confidence)
            && (self.points[0][0] - self.points[1][0]).hypot(self.points[0][1] - self.points[1][1])
                > 0.001
    }
}

/// At most four faces and a mapping from effect UVs back to source UVs.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFrame {
    /// Content fingerprint used by the bounded GPU cache.
    pub id: u64,
    /// Faces in source coordinates.
    pub faces: Vec<Face>,
    /// Original source width divided by height.
    pub aspect: f32,
    /// Effect UV to original source UV affine mapping.
    pub mapping: [f32; 6],
}
impl FaceFrame {
    /// Validate detected/cache values and fingerprint them independently of placement.
    pub fn new(faces: Vec<Face>, aspect: f32) -> Option<Self> {
        if faces.len() > 4
            || !aspect.is_finite()
            || aspect <= 0.0
            || faces.iter().any(|face| !face.valid())
        {
            return None;
        }
        let mut id = 0xcbf29ce484222325u64;
        for value in std::iter::once(&aspect).chain(faces.iter().flat_map(|face| {
            face.bounds
                .iter()
                .chain(face.points.iter().flatten())
                .chain(std::iter::once(&face.confidence))
        })) {
            for byte in value.to_bits().to_le_bytes() {
                id ^= u64::from(byte);
                id = id.wrapping_mul(0x100000001b3);
            }
        }
        Some(Self {
            id,
            faces,
            aspect,
            mapping: crate::subject::IDENTITY,
        })
    }
    /// Compose crop/flip/stabilization mapping without changing the detections.
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
    /// Uniform bytes: two affine rows, then four sets of eyes/nose/mouth/score.
    pub fn uniform(&self) -> Vec<u8> {
        let [a, b, c, d, e, f] = self.mapping;
        let mut values = vec![a, b, c, 0.0, d, e, f, 0.0];
        for index in 0..4 {
            match self.faces.get(index) {
                Some(face) => {
                    values.extend(face.points.iter().flatten().copied());
                    values.extend([face.confidence, self.aspect]);
                }
                None => values.extend([0.0; 12]),
            }
        }
        values.into_iter().flat_map(f32::to_le_bytes).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn facial_uniform_keeps_a_bounded_layout_and_composes_source_mapping() {
        let frame = FaceFrame::new(vec![], 16.0 / 9.0).unwrap();
        assert_eq!(frame.uniform().len(), 224);
        let flip = frame.remapped([-1.0, 0.0, 1.0, 0.0, 1.0, 0.0]).unwrap();
        let crop = flip.remapped([0.5, 0.0, 0.2, 0.0, 0.8, 0.1]).unwrap();
        assert_eq!(crop.mapping, [-0.5, 0.0, 0.8, 0.0, 0.8, 0.1]);
        assert_eq!(crop.id, frame.id);
        assert!(frame.remapped([f32::NAN; 6]).is_none());
        assert!(FaceFrame::new(vec![], f32::NAN).is_none());
    }
}
