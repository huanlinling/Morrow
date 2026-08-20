//! Runtime kernel — the heart of Morrow.
//!
//! Owns every per-runtime registry (mods, tick callbacks, events, host
//! API, quarantine, config, errors). One object, one lock, one lifetime:
//! creating a kernel on `morrow_init` and dropping it on `morrow_shutdown`
//! releases everything — no leaked global state across runtimes.

pub mod state;

use crate::error::ErrorChannel;
use crate::host_api::{
    CommandRegistry, ConfigStore, DispatchTables, HostApi, WorldSnapshot,
};
use crate::mod_loader::ModRegistry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use state::RuntimeState;

/// All per-runtime mutable state, behind one lock.
///
/// Kept in a single `Mutex` so a tick dispatch grabs exactly one lock
/// instead of the previous eight global maps. Callbacks are always
/// collected (cloned) out of the lock and invoked after it is released,
/// so a mod re-entering the runtime API can never deadlock.
pub struct RuntimeData {
    pub registry: ModRegistry,
    /// Dispatch tables behind an Arc: the per-tick loop snapshots them
    /// with a refcount bump instead of cloning every map.
    pub dispatch: Arc<DispatchTables>,
    pub lifecycle: LifecycleRegistry,
    pub errors: ErrorChannel,
    pub host_api: HostApi,
    pub commands: CommandRegistry,
    pub configs: ConfigStore,
    /// World snapshot refreshed once per tick (v0.14 PlayerSnapshot) —
    /// only while `snapshot_consumers > 0`.
    pub snapshot: Option<WorldSnapshot>,
    /// Reusable buffer for the snapshot upcall (taken/put back each tick).
    pub snapshot_buf: Vec<u8>,
    /// Live consumers of the per-tick WorldSnapshot refresh. Mod-facing
    /// snapshot query APIs increment this on first use; while zero the
    /// refresh upcall (and its O(players) serialization on the Java
    /// game thread) is skipped entirely. v0.16 ships no query API, so
    /// production pays nothing.
    pub snapshot_consumers: u32,
}

/// The Morrow Runtime kernel.
///
/// Created by `morrow_init`, destroyed by `morrow_shutdown`.
/// All state lives in [`RuntimeData`]; the state machine guards
/// the lifecycle.
pub struct RuntimeKernel {
    data: Mutex<RuntimeData>,
    state: Mutex<RuntimeState>,
}

/// Per-runtime lifecycle callbacks.
pub struct LifecycleRegistry {
    pub server_start: HashMap<String, unsafe extern "C" fn()>,
    pub server_stop: HashMap<String, unsafe extern "C" fn()>,
}

impl RuntimeKernel {
    /// Create a new runtime kernel in the Ready state.
    ///
    /// Called from `morrow_init` after ABI version check passes.
    pub fn new() -> Self {
        RuntimeKernel {
            data: Mutex::new(RuntimeData {
                registry: ModRegistry::new(),
                dispatch: Arc::new(DispatchTables::default()),
                lifecycle: LifecycleRegistry {
                    server_start: HashMap::new(),
                    server_stop: HashMap::new(),
                },
                errors: ErrorChannel::new(),
                host_api: HostApi::new(),
                commands: CommandRegistry::new(),
                configs: ConfigStore::new(),
                snapshot: None,
                snapshot_buf: Vec::new(),
                snapshot_consumers: 0,
            }),
            state: Mutex::new(RuntimeState::Ready),
        }
    }

    /// Lock the kernel's data for read/write.
    pub fn data(&self) -> std::sync::MutexGuard<'_, RuntimeData> {
        self.data.lock().unwrap()
    }

    /// Current runtime state.
    #[allow(dead_code)] // used in M3+ for state checks
    pub fn state(&self) -> RuntimeState {
        *self.state.lock().unwrap()
    }

    /// Begin shutdown. Transitions Ready → ShuttingDown.
    ///
    /// Returns `Err` with the current state if the transition is illegal.
    pub fn begin_shutdown(&self) -> Result<(), RuntimeState> {
        let mut state = self.state.lock().unwrap();
        let target = RuntimeState::ShuttingDown;
        if state.can_transition_to(target) {
            *state = target;
            Ok(())
        } else {
            Err(*state)
        }
    }

    /// Complete shutdown. Transitions ShuttingDown → Dead.
    pub fn finish_shutdown(&self) -> Result<(), RuntimeState> {
        let mut state = self.state.lock().unwrap();
        let target = RuntimeState::Dead;
        if state.can_transition_to(target) {
            *state = target;
            Ok(())
        } else {
            Err(*state)
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
        let rt = RuntimeKernel::new();
        assert_eq!(rt.state(), RuntimeState::Ready);

        assert!(rt.begin_shutdown().is_ok());
        assert_eq!(rt.state(), RuntimeState::ShuttingDown);

        assert!(rt.finish_shutdown().is_ok());
        assert_eq!(rt.state(), RuntimeState::Dead);
    }

    #[test]
    fn test_double_shutdown_is_error() {
        let rt = RuntimeKernel::new();
        rt.begin_shutdown().unwrap();
        rt.finish_shutdown().unwrap();

        // Second shutdown should fail — already dead
        assert!(rt.begin_shutdown().is_err());
    }

    #[test]
    fn test_cycle_10_times() {
        // Simulate init → shutdown × 10 using fresh kernels each time
        for _ in 0..10 {
            let rt = RuntimeKernel::new();
            rt.begin_shutdown().unwrap();
            rt.finish_shutdown().unwrap();
        }
    }

    #[test]
    fn test_shutdown_drops_all_registries() {
        // Fresh kernel has empty registries (the old 8-global-map design
        // leaked these; here they live and die with the kernel).
        let rt = RuntimeKernel::new();
        let data = rt.data();
        assert_eq!(data.registry.len(), 0);
        assert!(data.dispatch.events.player_join.is_empty());
        assert_eq!(data.dispatch.quarantined.len(), 0);
        assert!(!data.commands.dispatch("nope", ""));
    }
}
