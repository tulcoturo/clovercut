// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small local face detector and validated five-point source geometry.
//!
//! YuNet model: OpenCV Zoo, MIT, SHA-256
//! 8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4.
//! Coordinates are source fractions, independent of output size or crop.

pub use concat_core::face::Face;

fn overlap(a: &Face, b: &Face) -> f32 {
    let [ax, ay, aw, ah] = a.bounds;
    let [bx, by, bw, bh] = b.bounds;
    let area = ((ax + aw).min(bx + bw) - ax.max(bx)).max(0.0)
        * ((ay + ah).min(by + bh) - ay.max(by)).max(0.0);
    area / (aw * ah + bw * bh - area).max(f32::EPSILON)
}

/// Decode YuNet's three feature-pyramid levels and suppress duplicate boxes.
/// Output tensors are cls8/16/32, obj8/16/32, bbox8/16/32, kps8/16/32.
pub fn decode(outputs: &[Vec<f32>], width: u32, height: u32) -> Result<Vec<Face>, String> {
    if outputs.len() != 12 || width == 0 || height == 0 {
        return Err("face: invalid output layout".into());
    }
    let mut faces = Vec::new();
    for (level, stride) in [8u32, 16, 32].into_iter().enumerate() {
        let (cols, rows) = (width / stride, height / stride);
        let count = (cols * rows) as usize;
        if outputs[level].len() != count
            || outputs[level + 3].len() != count
            || outputs[level + 6].len() != count * 4
            || outputs[level + 9].len() != count * 10
        {
            return Err("face: unexpected tensor shape".into());
        }
        for i in 0..count {
            let confidence =
                (outputs[level][i].clamp(0.0, 1.0) * outputs[level + 3][i].clamp(0.0, 1.0)).sqrt();
            if !confidence.is_finite() || confidence < 0.7 {
                continue;
            }
            let (x, y) = ((i as u32 % cols) as f32, (i as u32 / cols) as f32);
            let box_values = &outputs[level + 6][i * 4..i * 4 + 4];
            let (bw, bh) = (
                box_values[2].exp() * stride as f32 / width as f32,
                box_values[3].exp() * stride as f32 / height as f32,
            );
            let mut points = [[0.0; 2]; 5];
            for (j, point) in points.iter_mut().enumerate() {
                *point = [
                    (outputs[level + 9][i * 10 + j * 2] + x) * stride as f32 / width as f32,
                    (outputs[level + 9][i * 10 + j * 2 + 1] + y) * stride as f32 / height as f32,
                ];
            }
            let face = Face {
                bounds: [
                    (box_values[0] + x) * stride as f32 / width as f32 - bw * 0.5,
                    (box_values[1] + y) * stride as f32 / height as f32 - bh * 0.5,
                    bw,
                    bh,
                ],
                points,
                confidence,
            };
            if face.valid() {
                faces.push(face);
            }
        }
    }
    faces.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    let mut kept: Vec<Face> = Vec::new();
    for face in faces {
        if kept.iter().all(|other| overlap(&face, other) < 0.3) {
            kept.push(face);
            if kept.len() == 4 {
                break;
            }
        }
    }
    Ok(kept)
}

/// Bundled YuNet inference. CPU fallback is provided by the common ONNX runtime.
#[cfg(feature = "infer")]
pub struct Detector(std::sync::Mutex<crate::runtime::Model>);
#[cfg(feature = "infer")]
impl Detector {
    /// Load the bundled, checksum-pinned model without a network request.
    pub fn load() -> Result<Self, String> {
        Ok(Self(std::sync::Mutex::new(
            crate::runtime::Model::from_bytes(include_bytes!(
                "../assets/yunet/face_detection_yunet_2023mar.onnx"
            ))?,
        )))
    }
    /// Aspect-preserving decode size, bounded to the model's 640 pixel edge.
    pub fn input_size(width: u32, height: u32) -> (u32, u32) {
        let scale = 640.0 / width.max(height).max(1) as f64;
        (
            ((width.max(1) as f64 * scale).round() as u32).max(1),
            ((height.max(1) as f64 * scale).round() as u32).max(1),
        )
    }
    /// Detect at a bounded resolution. RGB becomes the BGR planes expected by YuNet.
    pub fn detect(&self, frame: &concat_core::frame::Frame) -> Result<Vec<Face>, String> {
        let (w, h) = (640u32, 640u32);
        let (image_w, image_h) = Self::input_size(frame.width(), frame.height());
        let mut input = vec![0.0; (w * h * 3) as usize];
        for y in 0..image_h {
            for x in 0..image_w {
                let pixel = frame
                    .pixel(x * frame.width() / image_w, y * frame.height() / image_h)
                    .ok_or("face: empty frame")?;
                for c in 0..3 {
                    input[(c as u32 * w * h + y * w + x) as usize] = f32::from(pixel[2 - c]);
                }
            }
        }
        let outputs = self.0.lock().map_err(|_| "face model poisoned")?.run(
            vec![crate::runtime::Input {
                name: "input",
                dims: vec![1, 3, h as usize, w as usize],
                data: input.into(),
            }],
            &[
                "cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16",
                "bbox_32", "kps_8", "kps_16", "kps_32",
            ],
        )?;
        let mut faces = decode(
            &outputs.into_iter().map(|out| out.data).collect::<Vec<_>>(),
            w,
            h,
        )?;
        let scale = [w as f32 / image_w as f32, h as f32 / image_h as f32];
        for face in &mut faces {
            for (i, value) in face.bounds.iter_mut().enumerate() {
                *value *= scale[i % 2];
            }
            for point in &mut face.points {
                point[0] *= scale[0];
                point[1] *= scale[1];
            }
        }
        faces.retain(Face::valid);
        Ok(faces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "infer")]
    #[test]
    fn bundled_detector_finds_eyes_and_nose_in_a_real_reference_without_downloads() {
        let mut reader = png::Decoder::new(std::io::Cursor::new(include_bytes!(
            "../../concat-host/assets/card-subject.png"
        )))
        .read_info()
        .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let (w, h) = (502u32, 282u32);
        let mut crop = vec![0; (w * h * 4) as usize];
        for y in 0..h {
            let start = ((y * info.width + 585) * 4) as usize;
            let target = (y * w * 4) as usize;
            crop[target..target + (w * 4) as usize]
                .copy_from_slice(&pixels[start..start + (w * 4) as usize]);
        }
        let frame = concat_core::frame::Frame::from_rgba(w, h, crop).unwrap();
        let detector = Detector::load().unwrap();
        let faces = detector.detect(&frame).unwrap();
        assert!(
            faces
                .iter()
                .any(|face| (face.points[2][0] - 0.49).hypot(face.points[2][1] - 0.52) < 0.12),
            "expected reference face, got {faces:?}"
        );
        assert!(
            detector
                .detect(&concat_core::frame::Frame::black(320, 320))
                .unwrap()
                .is_empty(),
            "blank pictures must not acquire accessories"
        );
    }

    #[test]
    fn malformed_model_answers_are_rejected() {
        assert!(decode(&[], 320, 320).is_err());
        assert!(decode(&vec![vec![]; 12], 320, 320).is_err());
        let mut face = Face {
            bounds: [0.2, 0.2, 0.5, 0.5],
            points: [
                [0.3, 0.4],
                [0.6, 0.4],
                [0.45, 0.5],
                [0.35, 0.6],
                [0.55, 0.6],
            ],
            confidence: 0.9,
        };
        assert!(face.valid());
        face.points[0][0] = f32::NAN;
        assert!(!face.valid());
    }
}
