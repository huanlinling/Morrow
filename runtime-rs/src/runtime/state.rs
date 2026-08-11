//! Runtime state machine.
//!
//! Tracks the lifecycle of a Ferrum Runtime kernel. The "does not exist"
//! state is represented by absence from the global registry — see
//! [`super::RUNTIMES`]. Once created, a kernel proceeds linearly through
//! its three states.

/// A stage in the Ferrum Runtime lifecycle.
///
/// A kernel is created in the [`Ready`](RuntimeState::Ready) state
/// and destroyed after reaching [`Dead`](RuntimeState::Dead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// `ferrum_init` succeeded. Ready for mod loading.
    Ready,
    /// `ferrum_shutdown` called. Cleaning up resources.
    ShuttingDown,
    /// Shutdown complete. The kernel will be removed from the registry.
    Dead,
}

impl RuntimeState {
    /// Check whether a transition from `self` to `target` is legal.
    pub fn can_transition_to(self, target: RuntimeState) -> bool {
        matches!(
            (self, target),
            (RuntimeState::Ready, RuntimeState::ShuttingDown)
                | (RuntimeState::ShuttingDown, RuntimeState::Dead)
        )
    }
}

impl std::fmt::Display for RuntimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeState::Ready => write!(f, "READY"),
            RuntimeState::ShuttingDown => write!(f, "SHUTTING_DOWN"),
            RuntimeState::Dead => write!(f, "DEAD"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(RuntimeState::Ready.can_transition_to(RuntimeState::ShuttingDown));
        assert!(RuntimeState::ShuttingDown.can_transition_to(RuntimeState::Dead));
    }

    #[test]
    fn test_invalid_transitions() {
        // Cannot skip
        assert!(!RuntimeState::Ready.can_transition_to(RuntimeState::Dead));
        // Cannot go backwards
        assert!(!RuntimeState::ShuttingDown.can_transition_to(RuntimeState::Ready));
        assert!(!RuntimeState::Dead.can_transition_to(RuntimeState::Ready));
        assert!(!RuntimeState::Dead.can_transition_to(RuntimeState::ShuttingDown));
    }
}
