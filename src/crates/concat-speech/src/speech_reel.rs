//! Speech-only audio with a mapping back to the original clip timeline.

use std::borrow::Cow;

/// The sample rate whisper expects.
pub(crate) const SAMPLE_RATE: u32 = 16_000;

/// Sound kept before a span of speech, and after it. Silero's edges are
/// tight, and it misses a quiet start outright: the opening words of a
/// sentence can come a second and a half ahead of where Silero heard speech
/// begin, and without it whisper may lose the words.
pub(crate) const LEAD_IN: f64 = 1.5;
/// Sound kept after a span of speech.
pub(crate) const TAIL: f64 = 0.5;

/// One stretch of the sound in the reel: `len` samples, `at` into the reel
/// and `from` into the sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Piece {
    /// Offset inside the reel audio in samples.
    pub(crate) at: usize,
    /// Offset into the source audio in samples.
    pub(crate) from: usize,
    /// Length of this stretch in samples.
    pub(crate) len: usize,
}

/// What whisper is given to hear: the speech of the sound, cut out and
/// laid end to end, and where each piece of it came from - so a time in
/// what whisper heard can be told as a time in the sound.
#[derive(Debug)]
pub(crate) struct SpeechReel<'a> {
    /// Audio samples passed to whisper.
    pub(crate) samples: Cow<'a, [f32]>,
    /// Segments mapped back to original timeline positions.
    pub(crate) pieces: Vec<Piece>,
}

impl<'a> SpeechReel<'a> {
    /// The whole of the sound, as it is.
    pub(crate) fn whole(samples: &'a [f32]) -> Self {
        SpeechReel {
            pieces: vec![Piece {
                at: 0,
                from: 0,
                len: samples.len(),
            }],
            samples: Cow::Borrowed(samples),
        }
    }

    /// The `speech` of `samples` with [`LEAD_IN`] before each span and
    /// [`TAIL`] after it, end to end. Spans whose padding meets are one
    /// piece with the pause between kept as it was said, so a pause of
    /// two seconds or more is cut to two seconds of its own sound, and a
    /// shorter one is not cut at all.
    pub(crate) fn cut(samples: &[f32], speech: &[(f64, f64)]) -> SpeechReel<'static> {
        let rate = f64::from(SAMPLE_RATE);
        let to_sample = |seconds: f64| ((seconds.max(0.0) * rate) as usize).min(samples.len());
        let mut sorted = speech.to_vec();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut stretches: Vec<(usize, usize)> = Vec::new();
        for (start, end) in sorted {
            if !start.is_finite() || !end.is_finite() || end <= start || end <= 0.0 {
                continue;
            }
            let (from, to) = (to_sample(start - LEAD_IN), to_sample(end + TAIL));
            if to <= from {
                continue;
            }
            match stretches.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => stretches.push((from, to)),
            }
        }
        let mut reel = Vec::new();
        let mut pieces = Vec::with_capacity(stretches.len());
        for (from, to) in stretches {
            pieces.push(Piece {
                at: reel.len(),
                from,
                len: to - from,
            });
            reel.extend_from_slice(&samples[from..to]);
        }
        SpeechReel {
            samples: Cow::Owned(reel),
            pieces,
        }
    }

    /// Backward-compatible constructor that creates a cut reel from speech spans,
    /// or returns the whole audio if no valid speech spans are found.
    pub(crate) fn new(samples: &'a [f32], speech: &[(f64, f64)]) -> Self {
        let reel = Self::cut(samples, speech);
        if reel.pieces.is_empty() {
            Self::whole(samples)
        } else {
            reel
        }
    }

    /// Audio samples in the reel.
    pub(crate) fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Total duration of the reel in seconds.
    pub(crate) fn seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(SAMPLE_RATE)
    }

    /// Seconds into the reel as seconds into the sound. Past the end is
    /// the end of the last piece.
    pub(crate) fn to_source(&self, seconds: f64) -> f64 {
        let rate = f64::from(SAMPLE_RATE);
        let at = (seconds.max(0.0) * rate).round() as usize;
        self.pieces
            .iter()
            .find(|piece| at <= piece.at + piece.len)
            .or(self.pieces.last())
            .map_or(seconds, |piece| {
                (piece.from + at.clamp(piece.at, piece.at + piece.len) - piece.at) as f64 / rate
            })
    }

    /// Map a time interval from reel seconds to original audio seconds.
    pub(crate) fn source_span(&self, start: f64, end: f64) -> (f64, f64) {
        (self.to_source(start), self.to_source(end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_real_samples_and_maps_both_sides_of_a_seam() {
        let samples: Vec<_> = (0..320_000).map(|n| n as f32).collect();
        let reel = SpeechReel::cut(&samples, &[(12.0, 13.0), (2.0, 3.0), (4.0, 5.0)]);
        assert_eq!(reel.seconds(), 8.0);
        assert_eq!(&reel.samples()[..80_000], &samples[8_000..88_000]);
        assert_eq!(&reel.samples()[80_000..], &samples[168_000..216_000]);
        assert_eq!(reel.source_span(5.0, 5.0), (5.5, 5.5));
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
        let reel = SpeechReel::cut(&samples, &[(0.1, 2.0), (1.0, 8.0)]);
        assert_eq!(reel.seconds(), 4.0);
        let empty = SpeechReel::new(&[], &[]);
        assert_eq!(empty.source_span(0.0, 1.0), (0.0, 0.0));
    }
}
