//! Host API — function pointers provided by the Java host.
//!
//! During initialization, Java creates Panama upcall stubs and passes
//! them to Rust via a vtable struct. Rust mods call these to query
//! game state (player count, world info, etc.).
//!
//! ## Memory layout
//!
//! The vtable is a packed struct allocated by Java:
//!
//! ```text
//! offset 0: get_player_count() -> i32  (8 bytes = function pointer)
//! ```
//!
//! Future fields are appended at higher offsets.

use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Runtime API — passed to mods during init
// ---------------------------------------------------------------------------

/// Function table passed to `ferrum_mod_init(api: *const RuntimeApi)`.
///
/// Mods use this to call back into the runtime for host API queries
/// without needing to see runtime symbols (which aren't exported with
/// `RTLD_GLOBAL`).
#[repr(C)]
pub struct RuntimeApi {
    /// Query the online player count via Java upcall.
    /// Pass `runtime_handle = 0` for the first available runtime.
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
}

impl RuntimeApi {
    /// Create a RuntimeApi pointing to the current runtime's exports.
    pub fn new() -> Self {
        RuntimeApi {
            get_player_count: crate::ferrum_get_player_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Host Vtable (from Java)
// ---------------------------------------------------------------------------

/// Function pointer type: returns the current online player count.
type GetPlayerCountFn = unsafe extern "C" fn() -> i32;

/// Vtable passed from Java during init.
///
/// Safety: all function pointers must be valid upcall stubs
/// created by the Panama Linker. They live as long as the Arena
/// they were allocated in (the runtime's lifetime).
#[repr(C)]
pub struct HostVtable {
    pub get_player_count: Option<GetPlayerCountFn>,
}

/// Thread-safe holder for the host vtable.
pub struct HostApi {
    vtable: Mutex<Option<HostVtable>>,
}

impl HostApi {
    pub fn new() -> Self {
        HostApi {
            vtable: Mutex::new(None),
        }
    }

    /// Store the vtable pointer from Java.
    pub fn set_vtable(&self, ptr: *const HostVtable) {
        unsafe {
            let vtable = ptr.read();
            *self.vtable.lock().unwrap() = Some(vtable);
        }
    }

    /// Call into Java to get the current online player count.
    ///
    /// Returns `None` if the vtable hasn't been registered yet
    /// or the upcall fails.
    pub fn get_player_count(&self) -> Option<i32> {
        let guard = self.vtable.lock().unwrap();
        let vtable = guard.as_ref()?;
        let func = vtable.get_player_count?;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func()
        }));

        match result {
            Ok(count) => Some(count),
            Err(_) => {
                eprintln!("[Ferrum] Host API: get_player_count upcall panicked");
                None
            }
        }
    }
}
