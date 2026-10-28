// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! Completion ownership for timeline-bound background work, without UI dependencies.
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// Immutable context for one request. Identities survive pane resets.
#[derive(Debug)]
pub struct Pending<T> {
    id: u64,
    timeline: String,
    value: T,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Completion<T> {
    /// Another request (or cancellation) owns the slot; leave it untouched.
    Obsolete,
    /// This request completed after its destination changed; consume it safely.
    ContextChanged,
    Ready(T),
}

impl<T> Pending<T> {
    pub fn begin(timeline: String, value: T) -> Self {
        Self {
            id: NEXT_REQUEST.fetch_add(1, Ordering::Relaxed),
            timeline,
            value,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn accepts(&self, id: u64, timeline: &str) -> bool {
        self.id == id && self.timeline == timeline
    }

    pub fn finish(slot: &mut Option<Self>, id: u64, timeline: &str) -> Completion<T> {
        if slot.as_ref().is_none_or(|pending| pending.id != id) {
            return Completion::Obsolete;
        }
        let pending = slot.take().expect("request ownership checked");
        if pending.timeline != timeline {
            return Completion::ContextChanged;
        }
        Completion::Ready(pending.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_completion_does_not_consume_replacement_even_on_the_same_timeline() {
        let previous = Pending::begin("timeline".into(), "old destination");
        let current = Pending::begin("timeline".into(), "new destination");
        let current_id = current.id();
        let mut slot = Some(current);
        assert_eq!(
            Pending::finish(&mut slot, previous.id(), "timeline"),
            Completion::Obsolete
        );
        assert!(slot.as_ref().unwrap().accepts(current_id, "timeline"));
        assert_eq!(
            Pending::finish(&mut slot, current_id, "timeline"),
            Completion::Ready("new destination")
        );
        assert!(slot.is_none());
        assert_eq!(
            Pending::finish(&mut slot, current_id, "timeline"),
            Completion::Obsolete
        );
    }

    #[test]
    fn switching_timeline_discards_the_owned_result_and_releases_its_slot() {
        let pending = Pending::begin("original".into(), "caption timings");
        let id = pending.id();
        assert!(!pending.accepts(id, "different"));
        let mut slot = Some(pending);
        assert_eq!(
            Pending::finish(&mut slot, id, "different"),
            Completion::ContextChanged
        );
        assert!(slot.is_none());
    }

    #[test]
    fn cancellation_cannot_reuse_an_old_identity_after_a_pane_reset() {
        let mut slot = Some(Pending::begin("timeline".into(), "cancelled"));
        let id = slot.as_ref().unwrap().id();
        slot = None;
        assert_eq!(
            Pending::finish(&mut slot, id, "timeline"),
            Completion::Obsolete
        );
        slot = Some(Pending::begin("timeline".into(), "new pane"));
        assert_ne!(slot.as_ref().unwrap().id(), id);
        assert_eq!(
            Pending::finish(&mut slot, id, "timeline"),
            Completion::Obsolete
        );
        let new_id = slot.as_ref().unwrap().id();
        assert_eq!(
            Pending::finish(&mut slot, new_id, "timeline"),
            Completion::Ready("new pane")
        );
    }
}
