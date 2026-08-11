//! Runtime API vtable — passed to mods during `ferrum_mod_init`.
//!
//! Mirrors `host_api::RuntimeApi` in runtime-rs. Both crates define
//! the same `#[repr(C)]` struct so the ABI matches.

/// Function table passed to the mod's entry point.
///
/// The mod receives a pointer to this struct during `ferrum_mod_init`
/// and uses it to call back into the runtime for host queries
/// (player count, world state, etc.).
#[repr(C)]
pub struct RuntimeApi {
    /// Query the online player count via Java upcall.
    /// Pass `runtime_handle = 0` for the first available runtime.
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
}
