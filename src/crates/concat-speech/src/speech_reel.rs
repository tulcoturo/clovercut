//! Speech-only audio with a mapping back to the original clip timeline.

use std::borrow::Cow;

const RATE: f64 = 16_000.0;

pub(crate) struct SpeechReel<'a> {
    samples: Cow<'a, [f32]>,
    // Concatenated offset, source offset, sample count.
    pieces: Vec<(usize, usize, usize)>,
}

impl<'a> SpeechReel<'a> {
    pub(crate) fn new(samples: &'a [f32], speech: &[(f64, f64)]) -> Self {
        let mut spans: Vec<_> = speech
            .iter()
            .filter(|(start, end)| {
                start.is_finite() && end.is_finite() && *end > *start && *end > 0.0
            })
            .filter_map(|&(start, end)| {
                let from = ((start - 1.5).max(0.0) * RATE) as usize;
                let to = (((end + 0.5) * RATE) as usize).min(samples.len());
                (from < to).then_some((from, to))
            })
            .collect();
        spans.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (from, to) in spans {
            if let Some(last) = merged.last_mut()
                && from <= last.1
            {
                last.1 = last.1.max(to);
            } else {
                merged.push((from, to));
            }
        }
        // Detection can miss quiet speech. Preserve the existing Whisper fallback.
        if merged.is_empty() || merged == [(0, samples.len())] {
            return Self {
                samples: Cow::Borrowed(samples),
                pieces: vec![(0, 0, samples.len())],
            };
        }
        let mut audio = Vec::with_capacity(merged.iter().map(|(a, b)| b - a).sum());
        let mut pieces = Vec::with_capacity(merged.len());
        for (from, to) in merged {
            pieces.push((audio.len(), from, to - from));
            audio.extend_from_slice(&samples[from..to]);
        }
        Self {
            samples: Cow::Owned(audio),
            pieces,
        }
    }

    pub(crate) fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub(crate) fn seconds(&self) -> f64 {
        self.samples.len() as f64 / RATE
    }

    pub(crate) fn source_span(&self, start: f64, end: f64) -> (f64, f64) {
        (self.source_time(start, true), self.source_time(end, false))
    }

    fn source_time(&self, seconds: f64, start: bool) -> f64 {
        let at = ((seconds.max(0.0) * RATE).round() as usize).min(self.samples.len());
        // At a seam, a start belongs to the following piece and an end to
        // the preceding piece. One shared inclusive lookup shifts starts
        // backwards across the removed silence.
        let &(offset, source, len) = self
            .pieces
            .iter()
            .find(|&&(offset, _, len)| {
                if start {
                    at < offset + len
                } else {
                    at <= offset + len
                }
            })
            .unwrap_or_else(|| self.pieces.last().expect("a reel always has a piece"));
        (source + at.saturating_sub(offset).min(len)) as f64 / RATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_real_samples_and_maps_both_sides_of_a_seam() {
        let samples: Vec<_> = (0..320_000).map(|n| n as f32).collect();
        let reel = SpeechReel::new(&samples, &[(12.0, 13.0), (2.0, 3.0), (4.0, 5.0)]);
        assert_eq!(reel.seconds(), 8.0);
        assert_eq!(&reel.samples()[..80_000], &samples[8_000..88_000]);
        assert_eq!(&reel.samples()[80_000..], &samples[168_000..216_000]);
        assert_eq!(reel.source_span(5.0, 5.0), (10.5, 5.5));
        assert_eq!(reel.source_span(4.0, 6.0), (4.5, 11.5));
        assert_eq!(reel.source_span(-1.0, 99.0), (0.5, 13.5));
    }

    #[test]
    fn missing_or_unusable_detection_preserves_audio() {
        let samples = vec![0.25; 32_000];
        for spans in [vec![], vec![(f64::NAN, 1.0), (3.0, 2.0), (99.0, 100.0)]] {
            let reel = SpeechReel::new(&samples, &spans);
            assert!(matches!(reel.samples, Cow::Borrowed(_)));
            assert_eq!(reel.samples(), samples);
            assert_eq!(reel.source_span(0.5, 1.5), (0.5, 1.5));
        }
    }

    #[test]
    fn overlapping_padding_merges_without_duplicating_samples() {
        let samples = vec![1.0; 64_000];
        let reel = SpeechReel::new(&samples, &[(0.1, 2.0), (1.0, 8.0)]);
        assert!(matches!(reel.samples, Cow::Borrowed(_)));
        assert_eq!(reel.seconds(), 4.0);
        let empty = SpeechReel::new(&[], &[]);
        assert_eq!(empty.source_span(0.0, 1.0), (0.0, 0.0));
    }
}
