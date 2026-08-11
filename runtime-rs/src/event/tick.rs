//! Per-tick callback registry.
//!
//! Mods can optionally export `morrow_mod_tick(tick: u64)` to receive
//! tick events. The runtime discovers this symbol during mod loading
//! and registers it here.

use std::collections::HashMap;

/// A tick callback: `fn(tick_number: u64)`.
pub type TickCallback = unsafe extern "C" fn(u64);

/// Registry of tick callbacks, keyed by mod name.
pub struct TickRegistry {
    pub(crate) callbacks: HashMap<String, TickCallback>,
}

impl TickRegistry {
    pub fn new() -> Self {
        TickRegistry {
            callbacks: HashMap::new(),
        }
    }

    /// Register a tick callback for a mod.
    pub fn register(&mut self, mod_name: &str, callback: TickCallback) {
        self.callbacks.insert(mod_name.to_string(), callback);
    }

    /// Remove a mod's tick callback.
    #[allow(dead_code)]
    pub fn unregister(&mut self, mod_name: &str) {
        self.callbacks.remove(mod_name);
    }

    /// Fire all tick callbacks.
    ///
    /// Each callback is panic-isolated. Panicking mods are quarantined
    /// and will not receive future callbacks.
    ///
    /// Returns the names of mods that panicked.
    pub fn dispatch(&self, tick: u64) -> Vec<String> {
        let mut panicked = Vec::new();
        for (name, callback) in &self.callbacks {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                unsafe { callback(tick) };
            }));

            if let Err(payload) = result {
                let msg = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("<non-string panic>");
                eprintln!("[Morrow] Mod '{name}' panicked during tick {tick}: {msg}");
                panicked.push(name.clone());
            }
        }
        panicked
    }

    /// Number of registered tick callbacks.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.callbacks.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }

    /// Remove all callbacks.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.callbacks.clear();
    }
}

impl Default for TickRegistry {
    fn default() -> Self {
        Self::new()
    }
}
