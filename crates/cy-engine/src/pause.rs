//! Pausing the recording and skipping the next copy. Kept in memory only: a restart resumes.

/// Whether recording is paused, and how many coming copies to skip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PauseState {
    /// Paused until this time (milliseconds since the Unix epoch); `Some(u64::MAX)` means until
    /// resumed by hand.
    pub paused_until_ms: Option<u64>,
    /// Coming copies to skip.
    pub skip_next: u32,
}

/// What pausing decides about one copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    Record,
    Paused,
    Skipped,
}

impl PauseState {
    /// Whether recording is paused at `now_ms`.
    #[must_use]
    pub fn is_paused(&self, now_ms: u64) -> bool {
        self.paused_until_ms.is_some_and(|until| now_ms < until)
    }

    /// The state with an expired pause cleared.
    pub(crate) fn settled(self, now_ms: u64) -> Self {
        Self {
            paused_until_ms: self.paused_until_ms.filter(|until| now_ms < *until),
            ..self
        }
    }

    /// Decides one copy at `now_ms` and returns the next state.
    pub(crate) fn gate(self, now_ms: u64) -> (Self, Gate) {
        let state = self.settled(now_ms);
        if state.is_paused(now_ms) {
            return (state, Gate::Paused);
        }
        if state.skip_next > 0 {
            return (
                Self {
                    skip_next: state.skip_next - 1,
                    ..state
                },
                Gate::Skipped,
            );
        }
        (state, Gate::Record)
    }

    /// When a timed pause ends, if one is running (for the shell's single timer).
    pub(crate) fn wake_at(&self) -> Option<u64> {
        self.paused_until_ms.filter(|until| *until != u64::MAX)
    }
}

impl crate::Engine {
    /// Pauses recording until `until_ms`, or until [`crate::Engine::resume`] with `None`.
    /// Copies made while paused are never read.
    pub fn pause(&mut self, until_ms: Option<u64>) {
        self.pause = PauseState {
            paused_until_ms: Some(until_ms.unwrap_or(u64::MAX)),
            ..self.pause
        };
    }

    /// Resumes recording.
    pub fn resume(&mut self) {
        self.pause = PauseState {
            paused_until_ms: None,
            ..self.pause
        };
    }

    /// Skips the next copy (calling it twice skips two).
    pub fn skip_next(&mut self) {
        self.pause = PauseState {
            skip_next: self.pause.skip_next.saturating_add(1),
            ..self.pause
        };
    }

    /// The pause state at `now_ms` (an expired pause shows as not paused).
    #[must_use]
    pub fn pause_state(&self, now_ms: u64) -> PauseState {
        self.pause.settled(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timed_pause_ends_by_itself() {
        let state = PauseState {
            paused_until_ms: Some(1_000),
            skip_next: 0,
        };

        assert_eq!(state.gate(999).1, Gate::Paused);
        assert_eq!(state.gate(1_000), (PauseState::default(), Gate::Record));
        assert_eq!(state.wake_at(), Some(1_000));
    }

    #[test]
    fn an_open_pause_lasts_until_resumed_and_has_no_wake_time() {
        let state = PauseState {
            paused_until_ms: Some(u64::MAX),
            skip_next: 0,
        };

        assert_eq!(state.gate(u64::MAX - 1).1, Gate::Paused);
        assert_eq!(state.wake_at(), None);
    }

    #[test]
    fn skipping_counts_down() {
        let state = PauseState {
            paused_until_ms: None,
            skip_next: 2,
        };

        let (state, first) = state.gate(0);
        let (state, second) = state.gate(0);
        let (_, third) = state.gate(0);

        assert_eq!(
            (first, second, third),
            (Gate::Skipped, Gate::Skipped, Gate::Record)
        );
    }
}
