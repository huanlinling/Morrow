//! Context passed to the mod entry point.
//!
//! Provides access to runtime capabilities. Currently minimal —
//! will grow with event bus access, command registration, etc. in M5+.

/// Execution context for a Ferrum mod.
///
/// Created by the runtime and passed to `ferrum_mod_init`.
/// The mod uses this to register event listeners, access
/// capabilities, and interact with the Minecraft server.
pub struct Context {
    // Future: EventBus handle, CommandDispatcher handle, Config handle, etc.
    _private: (),
}

impl Context {
    /// Create a new context (called by generated code).
    #[doc(hidden)]
    pub fn new() -> Self {
        Context { _private: () }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
