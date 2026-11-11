// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Ownership and bounded caching for the monitor's background title painter.
use crate::request_owner::{Completion, Pending};

/// The document and visible cues a prepared set belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    /// Active timeline identity.
    pub timeline: String,
    /// Document revision.
    pub revision: u64,
    /// Preview width.
    pub width: u32,
    /// Preview height.
    pub height: u32,
    /// Visible title/shape identities, including when seeking backwards.
    pub visible: Vec<String>,
}

/// One running painter and one completed set; no queue of old playheads.
pub struct Preview<T> {
    pending: Option<Pending<Key>>,
    prepared: Option<(Key, Vec<T>)>,
}

impl<T> Default for Preview<T> {
    fn default() -> Self {
        Self {
            pending: None,
            prepared: None,
        }
    }
}

impl<T> Preview<T> {
    /// Ready results for precisely this document, size and set of cues.
    pub fn ready(&self, key: &Key) -> Option<&[T]> {
        self.prepared
            .as_ref()
            .filter(|(at, _)| at == key)
            .map(|(_, titles)| titles.as_slice())
    }

    /// Start only when idle and this set has not already completed, even
    /// when a failed paint returned an empty set.
    pub fn begin(&mut self, key: Key) -> Option<u64> {
        if self.pending.is_some() || self.ready(&key).is_some() {
            return None;
        }
        let pending = Pending::begin(key.timeline.clone(), key);
        let id = pending.id();
        self.pending = Some(pending);
        Some(id)
    }

    /// Release only this worker's slot. A stale timeline never receives
    /// its paintings, and a pre-reset result cannot consume a newer job.
    pub fn finish(&mut self, id: u64, timeline: &str, titles: Vec<T>) -> bool {
        match Pending::finish(&mut self.pending, id, timeline) {
            Completion::Ready(key) => self.prepared = Some((key, titles)),
            Completion::ContextChanged => {}
            Completion::Obsolete => return false,
        }
        true
    }

    /// Drop the previous project's results and completion ownership.
    pub fn clear(&mut self) {
        self.pending = None;
        self.prepared = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key() -> Key {
        Key {
            timeline: "main".into(),
            revision: 1,
            width: 640,
            height: 360,
            visible: vec!["cue-1".into()],
        }
    }
    #[test]
    fn scrubbing_coalesces_work_and_a_failed_title_does_not_spin() {
        let mut preview = Preview::<u8>::default();
        let first = key();
        let id = preview.begin(first.clone()).unwrap();
        let mut next = first.clone();
        next.visible = vec!["cue-2".into()];
        assert!(preview.begin(next.clone()).is_none());
        assert!(preview.finish(id, "main", vec![]));
        assert_eq!(preview.ready(&first), Some([].as_slice()));
        assert!(preview.begin(first).is_none());
        assert!(preview.ready(&next).is_none());
        assert!(preview.begin(next).is_some());
    }
    #[test]
    fn revisions_sizes_and_cue_changes_never_reuse_stale_results() {
        let mut preview = Preview::default();
        let original = key();
        let id = preview.begin(original.clone()).unwrap();
        preview.finish(id, "main", vec![42]);
        for changed in [
            Key {
                revision: 2,
                ..original.clone()
            },
            Key {
                width: 320,
                ..original.clone()
            },
            Key {
                visible: vec!["cue-2".into()],
                ..original.clone()
            },
        ] {
            assert!(preview.ready(&changed).is_none());
        }
        assert_eq!(
            preview.ready(&original),
            Some([42].as_slice()),
            "a backwards seek may reuse the same cue"
        );
    }
    #[test]
    fn closing_or_switching_projects_discards_delayed_completion() {
        let mut preview = Preview::<u8>::default();
        let old = preview.begin(key()).unwrap();
        preview.clear();
        let current = preview.begin(key()).unwrap();
        assert!(!preview.finish(old, "main", vec![1]));
        assert!(
            preview.begin(key()).is_none(),
            "new owner survives old completion"
        );
        assert!(preview.finish(current, "other-timeline", vec![2]));
        assert!(preview.ready(&key()).is_none());
        assert!(
            preview.begin(key()).is_some(),
            "changed context releases the slot"
        );
    }
}
