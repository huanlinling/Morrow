//! Runtime kernel — the heart of Morrow.
//!
//! Holds the runtime state and will eventually own the mod registry,
//! event bus, and capability table (added in later milestones).

pub mod state;

use state::RuntimeState;

/// The Morrow Runtime kernel.
///
/// Created by `morrow_init`, destroyed by `morrow_shutdown`.
/// In Milestone 1 this is intentionally minimal — just the state
/// machine. Mod registry, event bus, and capability system will
/// be added in M3/M4/M5.
pub struct RuntimeKernel {
    state: RuntimeState,
}

impl RuntimeKernel {
    /// Create a new runtime kernel in the Ready state.
    ///
    /// Called from `morrow_init` after ABI version check passes.
    pub fn new() -> Self {
        RuntimeKernel {
            state: RuntimeState::Ready,
        }
    }

    /// Current runtime state.
    #[allow(dead_code)] // used in M3+ for state checks
    pub fn state(&self) -> RuntimeState {
        self.state
    }

    /// Begin shutdown. Transitions Ready → ShuttingDown.
    ///
    /// Returns `Err` with the current state if the transition is illegal.
    pub fn begin_shutdown(&mut self) -> Result<(), RuntimeState> {
        let target = RuntimeState::ShuttingDown;
        if self.state.can_transition_to(target) {
            self.state = target;
            Ok(())
        } else {
            Err(self.state)
        }
    }

    /// Complete shutdown. Transitions ShuttingDown → Dead.
    pub fn finish_shutdown(&mut self) -> Result<(), RuntimeState> {
        let target = RuntimeState::Dead;
        if self.state.can_transition_to(target) {
            self.state = target;
            Ok(())
        } else {
            Err(self.state)
        }
    }
}

impl Default for RuntimeKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_lifecycle() {
        let mut rt = RuntimeKernel::new();
        assert_eq!(rt.state(), RuntimeState::Ready);

        assert!(rt.begin_shutdown().is_ok());
        assert_eq!(rt.state(), RuntimeState::ShuttingDown);

        assert!(rt.finish_shutdown().is_ok());
        assert_eq!(rt.state(), RuntimeState::Dead);
    }

    #[test]
    fn test_double_shutdown_is_error() {
        let mut rt = RuntimeKernel::new();
        rt.begin_shutdown().unwrap();
        rt.finish_shutdown().unwrap();

        // Second shutdown should fail — already dead
        assert!(rt.begin_shutdown().is_err());
    }

    #[test]
    fn test_cycle_10_times() {
        // Simulate init → shutdown × 10 using fresh kernels each time
        for _ in 0..10 {
            let mut rt = RuntimeKernel::new();
            rt.begin_shutdown().unwrap();
            rt.finish_shutdown().unwrap();
        }
    }
}
