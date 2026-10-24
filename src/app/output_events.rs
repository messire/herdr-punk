//! Coalesced `pane.output_changed` emission.
//!
//! PTY output arrives per read chunk; the JSON event history is a small ring
//! buffer shared by every subscriber. The tracker turns a stream of dirty
//! sources into at most one event per pane per [`OUTPUT_EVENT_MIN_INTERVAL`],
//! always carrying the latest observed content revision and never dropping the
//! trailing change of a burst.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::layout::PaneId;

pub(crate) const OUTPUT_EVENT_MIN_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
pub(crate) struct OutputEventTracker {
    panes: HashMap<PaneId, PaneOutputState>,
}

#[derive(Debug, Default, Clone, Copy)]
struct PaneOutputState {
    /// Revision carried by the last emitted event.
    emitted_revision: Option<u64>,
    emitted_at: Option<Instant>,
    /// Latest observed revision that has not been emitted yet.
    pending_revision: Option<u64>,
}

impl OutputEventTracker {
    /// Record that `pane` produced output and its content counter now reads
    /// `revision`. Odd counters mean a write is in flight; they are still
    /// accepted because the next observation supersedes them.
    pub(crate) fn note(&mut self, pane: PaneId, revision: u64) {
        let state = self.panes.entry(pane).or_default();
        if state.emitted_revision == Some(revision) {
            return;
        }
        state.pending_revision = Some(revision);
    }

    /// Drain every pane whose throttle window has elapsed. Returns the events
    /// to emit as `(pane, revision)`.
    pub(crate) fn due(&mut self, now: Instant) -> Vec<(PaneId, u64)> {
        let mut out = Vec::new();
        for (pane, state) in self.panes.iter_mut() {
            let Some(revision) = state.pending_revision else {
                continue;
            };
            let ready = state
                .emitted_at
                .is_none_or(|at| now.duration_since(at) >= OUTPUT_EVENT_MIN_INTERVAL);
            if !ready {
                continue;
            }
            state.pending_revision = None;
            state.emitted_revision = Some(revision);
            state.emitted_at = Some(now);
            out.push((*pane, revision));
        }
        out.sort_by_key(|(pane, _)| pane.raw());
        out
    }

    /// Earliest instant at which [`Self::due`] would return something.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.panes
            .values()
            .filter(|state| state.pending_revision.is_some())
            .map(|state| {
                state
                    .emitted_at
                    .map_or_else(Instant::now, |at| at + OUTPUT_EVENT_MIN_INTERVAL)
            })
            .min()
    }

    pub(crate) fn forget(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(n: u32) -> PaneId {
        PaneId::from_raw(n)
    }

    #[test]
    fn first_observation_is_due_immediately() {
        let mut tracker = OutputEventTracker::default();
        let now = Instant::now();
        tracker.note(pane(1), 2);
        assert_eq!(tracker.due(now), vec![(pane(1), 2)]);
        assert!(tracker.due(now).is_empty(), "nothing pending after drain");
    }

    #[test]
    fn burst_within_window_coalesces_to_latest_revision() {
        let mut tracker = OutputEventTracker::default();
        let t0 = Instant::now();
        tracker.note(pane(1), 2);
        assert_eq!(tracker.due(t0), vec![(pane(1), 2)]);

        tracker.note(pane(1), 4);
        tracker.note(pane(1), 6);
        assert!(tracker.due(t0 + Duration::from_millis(50)).is_empty());
        assert_eq!(
            tracker.next_deadline(),
            Some(t0 + OUTPUT_EVENT_MIN_INTERVAL)
        );
        assert_eq!(
            tracker.due(t0 + OUTPUT_EVENT_MIN_INTERVAL),
            vec![(pane(1), 6)],
            "trailing change is emitted once the window elapses"
        );
    }

    #[test]
    fn unchanged_revision_is_not_re_emitted() {
        let mut tracker = OutputEventTracker::default();
        let t0 = Instant::now();
        tracker.note(pane(1), 2);
        let _ = tracker.due(t0);
        tracker.note(pane(1), 2);
        assert!(tracker.due(t0 + Duration::from_secs(1)).is_empty());
        assert_eq!(tracker.next_deadline(), None);
    }

    #[test]
    fn panes_are_throttled_independently() {
        let mut tracker = OutputEventTracker::default();
        let t0 = Instant::now();
        tracker.note(pane(1), 2);
        let _ = tracker.due(t0);
        tracker.note(pane(1), 4);
        tracker.note(pane(2), 2);
        assert_eq!(
            tracker.due(t0 + Duration::from_millis(10)),
            vec![(pane(2), 2)]
        );
    }

    #[test]
    fn forget_drops_pending_work() {
        let mut tracker = OutputEventTracker::default();
        tracker.note(pane(1), 2);
        tracker.forget(pane(1));
        assert!(tracker.due(Instant::now()).is_empty());
        assert_eq!(tracker.next_deadline(), None);
    }
}
