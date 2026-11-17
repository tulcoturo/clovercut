// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Clovercut contributors
//! Sampling a source picture for the inspector eyedropper.

/// The colour about `(u, v)` of `frame`, in fractions of it: the mean of
/// the pixels within two of the one there - five by five - taken in light
/// and written back in the display encoding the frame and a colour knob
/// both hold, so a pick on grain or a codec's blocks lands on the colour
/// around it, not on one speck. None off the picture.
pub fn about(frame: &concat_core::frame::Frame, u: f64, v: f64) -> Option<[u8; 3]> {
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }
    if frame.depth() != concat_core::frame::Depth::Eight {
        return None;
    }
    let (width, height) = (frame.width() as i64, frame.height() as i64);
    if width == 0 || height == 0 {
        return None;
    }
    let x = ((u * width as f64) as i64).clamp(0, width - 1);
    let y = ((v * height as f64) as i64).clamp(0, height - 1);
    let pixels = frame.pixels();
    let mut sum = [0.0f64; 3];
    let mut count = 0.0;
    for row in (y - 2).max(0)..=(y + 2).min(height - 1) {
        for column in (x - 2).max(0)..=(x + 2).min(width - 1) {
            let at = ((row * width + column) * 4) as usize;
            let Some(pixel) = pixels.get(at..at + 3) else {
                continue;
            };
            for (total, byte) in sum.iter_mut().zip(pixel) {
                *total += (f64::from(*byte) / 255.0).powf(2.4);
            }
            count += 1.0;
        }
    }
    if count == 0.0 {
        return None;
    }
    Some(sum.map(|total| {
        ((total / count).powf(1.0 / 2.4) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    }))
}

/// Whether a colour has a channel standing clear of the other two - the
/// test a key holds a screen colour to - rather than being a grey.
pub fn has_hue([r, g, b]: [u8; 3]) -> bool {
    let mut channels = [r, g, b].map(|byte| (f64::from(byte) / 255.0).powf(2.4));
    channels.sort_by(f64::total_cmp);
    let [_, middle, most] = channels;
    most > 0.002 && most - middle >= 0.1 * most
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_source_colour_and_refuses_coordinates_outside_the_picture() {
        let mut frame = concat_core::Frame::transparent(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                frame.set_pixel(x, y, [12, 150, 40, 255]);
            }
        }
        for (u, v) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
            assert_eq!(about(&frame, u, v), Some([12, 150, 40]));
        }
        for (u, v) in [
            (-0.01, 0.5),
            (0.5, 1.01),
            (f64::NAN, 0.5),
            (0.5, f64::INFINITY),
        ] {
            assert_eq!(about(&frame, u, v), None);
        }
    }
    #[test]
    fn screen_hue_rejects_grey_and_dark_samples() {
        assert!(has_hue([0, 177, 64]));
        assert!(has_hue([0, 71, 187]));
        for rgb in [
            [0, 0, 0],
            [2, 1, 1],
            [128, 128, 128],
            [128, 127, 128],
            [255, 255, 255],
        ] {
            assert!(!has_hue(rgb));
        }
    }
}
