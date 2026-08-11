//! Ferrum Runtime Core
//!
//! This is the Rust cdylib loaded by the Java host via Panama FFM.
//! All public symbols are `extern "C"` and use the platform C ABI.

mod abi;
mod error;
mod event;
mod host_api;
mod mod_loader;
mod panic;
mod runtime;

use abi::handles::HandleTable;
use mod_loader::ModRegistry;
use runtime::RuntimeKernel;
use std::path::Path;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Global registries
// ---------------------------------------------------------------------------

/// Registry of live runtime kernels, keyed by opaque handle.
static RUNTIMES: LazyLock<HandleTable<RuntimeKernel>> =
    LazyLock::new(HandleTable::new);

// ---------------------------------------------------------------------------
// M0: add — first proof of the Panama bridge (retained)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    panic::ffi_boundary(0, || a + b)
}

// ---------------------------------------------------------------------------
// M1: Runtime lifecycle
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_init(abi_version: u32) -> u64 {
    panic::ffi_boundary(0, || {
        if !abi::is_abi_compatible(abi_version, abi::ABI_VERSION) {
            eprintln!(
                "[Ferrum] ABI version mismatch: requested {abi_version:#010x}, \
                 runtime {:#010x}",
                abi::ABI_VERSION
            );
            return 0;
        }

        let kernel = RuntimeKernel::new();
        let handle = RUNTIMES.insert(kernel);

        // Create a mod registry for this runtime
        register_mod_registry(handle.as_u64());

        eprintln!(
            "[Ferrum] Runtime initialized (ABI {abi_version:#010x}, handle={})",
            handle.as_u64()
        );
        handle.as_u64()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_shutdown(runtime_handle: u64) -> u32 {
    panic::ffi_boundary(abi::RESULT_ERR_PANIC, || {
        let handle = match abi::handles::Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => {
                eprintln!("[Ferrum] ferrum_shutdown: invalid handle 0");
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        let mut kernel = match RUNTIMES.remove(handle) {
            Some(k) => k,
            None => {
                eprintln!(
                    "[Ferrum] ferrum_shutdown: handle {} not found",
                    handle.as_u64()
                );
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        if let Err(state) = kernel.begin_shutdown() {
            eprintln!("[Ferrum] ferrum_shutdown: illegal state transition from {state}");
            RUNTIMES.insert(kernel);
            return abi::RESULT_ERR_WRONG_STATE;
        }

        // Unload all mods
        if let Some(registry) = remove_mod_registry(handle.as_u64()) {
            let count = registry.len();
            if count > 0 {
                eprintln!("[Ferrum] Unloaded {count} mod(s)");
            }
            // registry drops here → libraries unloaded
        }

        if let Err(state) = kernel.finish_shutdown() {
            eprintln!("[Ferrum] ferrum_shutdown: finish_shutdown failed from {state}");
            return abi::RESULT_ERR_WRONG_STATE;
        }

        eprintln!("[Ferrum] Runtime shut down (handle={})", handle.as_u64());
        abi::RESULT_OK
    })
}

// ---------------------------------------------------------------------------
// M3: Mod loading
// ---------------------------------------------------------------------------

/// Load a `.ferrum` package into the given runtime.
///
/// # Parameters
/// - `runtime_handle`: handle from [`ferrum_init`]
/// - `path_ptr`: pointer to UTF-8 path string
/// - `path_len`: length of the path string in bytes
///
/// # Returns
/// - `0` on success
/// - Non-zero error code on failure
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_load_mod(
    runtime_handle: u64,
    path_ptr: *const u8,
    path_len: u32,
) -> u32 {
    panic::ffi_boundary(abi::RESULT_ERR_PANIC, || {
        let handle = match abi::handles::Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => return abi::RESULT_ERR_INVALID_HANDLE,
        };

        // Read path string from FFI
        let path_bytes = unsafe {
            std::slice::from_raw_parts(path_ptr, path_len as usize)
        };
        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("[Ferrum] ferrum_load_mod: invalid UTF-8 in path");
                return abi::RESULT_ERR_UNKNOWN;
            }
        };

        let package_path = Path::new(path_str);
        eprintln!("[Ferrum] ferrum_load_mod: {}", package_path.display());

        // Look up the mod registry for this runtime handle.
        match MOD_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
            Some(registry) => {
                match mod_loader::load_package(package_path, registry) {
                    Ok((name, exports)) => {
                        if let Some(cb) = exports.tick_callback {
                            if let Some(reg) = TICK_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.register(&name, cb);
                                eprintln!("[Ferrum]   Registered tick callback for '{name}'");
                            }
                        }
                        if let Some(cb) = exports.server_start_callback {
                            if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.server_start.insert(name.clone(), cb);
                                eprintln!("[Ferrum]   Registered server_start for '{name}'");
                            }
                        }
                        if let Some(cb) = exports.server_stop_callback {
                            if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.server_stop.insert(name.clone(), cb);
                                eprintln!("[Ferrum]   Registered server_stop for '{name}'");
                            }
                        }

                        eprintln!("[Ferrum] Mod '{name}' loaded successfully");
                        abi::RESULT_OK
                    }
                    Err(e) => {
                        eprintln!("[Ferrum] Failed to load mod: {e}");
                        record_error(handle.as_u64(), format!("ferrum_load_mod: {e}"));
                        abi::RESULT_ERR_UNKNOWN
                    }
                }
            }
            None => {
                eprintln!("[Ferrum] ferrum_load_mod: no mod registry for runtime {handle:?}");
                abi::RESULT_ERR_INVALID_HANDLE
            }
        }
    })
}

// ---------------------------------------------------------------------------
// M4: Tick dispatch
// ---------------------------------------------------------------------------

/// Drive one tick cycle — dispatches to all registered mod tick callbacks.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_tick(runtime_handle: u64, tick_number: u64) {
    panic::ffi_boundary((), || {
        let handle = match abi::handles::Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => return,
        };

        if let Some(registry) = TICK_REGISTRIES.lock().unwrap().get(&handle.as_u64()) {
            registry.dispatch(tick_number);
        }
    })
}

// ---------------------------------------------------------------------------
// Per-runtime registries (mods + tick callbacks)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;
use event::tick::TickRegistry;

static MOD_REGISTRIES: LazyLock<Mutex<HashMap<u64, ModRegistry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static TICK_REGISTRIES: LazyLock<Mutex<HashMap<u64, TickRegistry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static LIFECYCLE_REGISTRIES: LazyLock<Mutex<HashMap<u64, LifecycleRegistry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static ERROR_CHANNELS: LazyLock<Mutex<HashMap<u64, error::ErrorChannel>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static HOST_APIS: LazyLock<Mutex<HashMap<u64, host_api::HostApi>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-runtime lifecycle callbacks.
pub struct LifecycleRegistry {
    pub server_start: HashMap<String, unsafe extern "C" fn()>,
    pub server_stop: HashMap<String, unsafe extern "C" fn()>,
}

fn register_mod_registry(handle: u64) {
    MOD_REGISTRIES.lock().unwrap().insert(handle, ModRegistry::new());
    TICK_REGISTRIES.lock().unwrap().insert(handle, TickRegistry::new());
    LIFECYCLE_REGISTRIES.lock().unwrap().insert(handle, LifecycleRegistry {
        server_start: HashMap::new(),
        server_stop: HashMap::new(),
    });
    ERROR_CHANNELS.lock().unwrap().insert(handle, error::ErrorChannel::new());
    HOST_APIS.lock().unwrap().insert(handle, host_api::HostApi::new());
}

fn remove_mod_registry(handle: u64) -> Option<ModRegistry> {
    TICK_REGISTRIES.lock().unwrap().remove(&handle);
    LIFECYCLE_REGISTRIES.lock().unwrap().remove(&handle);
    ERROR_CHANNELS.lock().unwrap().remove(&handle);
    MOD_REGISTRIES.lock().unwrap().remove(&handle)
}

/// Register the Java host function table (upcall stubs).
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_register_host_api(
    runtime_handle: u64,
    vtable_ptr: *const host_api::HostVtable,
) {
    panic::ffi_boundary((), || {
        if vtable_ptr.is_null() { return; }
        if let Some(api) = HOST_APIS.lock().unwrap().get(&runtime_handle) {
            api.set_vtable(vtable_ptr);
            eprintln!("[Ferrum] Host API registered");
        }
    })
}

/// Get the online player count via Java upcall.
///
/// If `runtime_handle` is 0, uses the first available runtime.
/// Returns -1 if the host API isn't registered yet.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_get_player_count(runtime_handle: u64) -> i32 {
    panic::ffi_boundary(-1, || {
        let apis = HOST_APIS.lock().unwrap();
        let api = if runtime_handle == 0 {
            apis.values().next()
        } else {
            apis.get(&runtime_handle)
        };
        api.and_then(|a| a.get_player_count()).unwrap_or(-1)
    })
}

/// Record an error for a runtime (used internally by lifecycle operations).
fn record_error(handle: u64, msg: impl Into<String>) {
    if let Some(ch) = ERROR_CHANNELS.lock().unwrap().get(&handle) {
        ch.push(msg);
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_handle_count() -> u64 {
    panic::ffi_boundary(0, || RUNTIMES.len() as u64)
}

/// Return the number of loaded mods across all runtimes.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_count() -> u64 {
    panic::ffi_boundary(0, || {
        MOD_REGISTRIES.lock().unwrap().values().map(|r| r.len()).sum::<usize>() as u64
    })
}

// ---------------------------------------------------------------------------
// Lifecycle dispatch
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_dispatch_server_start(runtime_handle: u64) {
    panic::ffi_boundary((), || {
        if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get(&runtime_handle) {
            for (name, cb) in &reg.server_start {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb() };
                }));
                if let Err(p) = result {
                    eprintln!("[Ferrum] Mod '{name}' panicked in server_start: {:?}",
                        p.downcast_ref::<&str>().unwrap_or(&"<unknown>"));
                }
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_dispatch_server_stop(runtime_handle: u64) {
    panic::ffi_boundary((), || {
        if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get(&runtime_handle) {
            for (_name, cb) in &reg.server_stop {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb() };
                }));
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Error Channel
// ---------------------------------------------------------------------------

/// Get the handle of the oldest pending error, or 0 if no errors.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_last_error(runtime_handle: u64) -> u64 {
    panic::ffi_boundary(0, || {
        ERROR_CHANNELS
            .lock().unwrap()
            .get(&runtime_handle)
            .and_then(|ch| ch.peek())
            .map(|e| e.id)
            .unwrap_or(0)
    })
}

/// Read an error message into the provided buffer.
///
/// Returns the number of bytes written (excluding null terminator),
/// or 0 if the error handle is not found.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_error_message(
    error_handle: u64,
    runtime_handle: u64,
    buffer_ptr: *mut u8,
    buffer_cap: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        let channels = ERROR_CHANNELS.lock().unwrap();
        let ch = match channels.get(&runtime_handle) {
            Some(ch) => ch,
            None => return 0,
        };

        let record = match ch.take(error_handle) {
            Some(r) => r,
            None => return 0,
        };

        let msg = record.message;
        let bytes = msg.as_bytes();
        let len = bytes.len().min(buffer_cap as usize);

        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer_ptr, len);
        }

        len as u32
    })
}
