use tezedge_state::proposer::Events;

use crate::fake_event::{FakeEvent, FakeEventRef};

/// Fake Events.
///
/// Events with time difference of less than a [EVENT_GROUP_BOUNDARY]
/// will be grouped together.
///
/// # Panics
///
/// Time in the events must be ascending or it will panic.
#[derive(Debug, Clone)]
pub struct FakeEvents {
    pub events: Vec<FakeEvent>,
    pub limit: usize,
}

impl FakeEvents {
    pub fn new() -> Self {
        Self {
            events: vec![],
            // will be later set from proposer using [Events::set_limit].
            limit: 0,
        }
    }

    /// Try to push the event if we didn't accumulate more events than
    /// the set `limit`. If we did, passed event will be return back
    /// as a `Result::Err(FakeEvent)`.
    pub fn try_push(&mut self, event: FakeEvent) -> Result<&mut Self, FakeEvent> {
        if self.events.len() >= self.limit {
            Err(event)
        } else {
            self.events.push(event);
            Ok(self)
        }
    }
}

impl Events for FakeEvents {
    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }
}

#[derive(Debug, Clone)]
pub struct FakeEventsIter<'a> {
    events: &'a [FakeEvent],
}

impl<'a> Iterator for FakeEventsIter<'a> {
    type Item = FakeEventRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.events.len() == 0 {
            return None;
        }
        let event = &self.events[0];
        self.events = &self.events[1..];
        Some(event.as_event_ref())
    }
}

impl<'a> IntoIterator for &'a FakeEvents {
    type Item = FakeEventRef<'a>;
    type IntoIter = FakeEventsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        FakeEventsIter {
            events: &self.events,
        }
    }
}
