//! Morrow Runtime Core
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
pub extern "C" fn morrow_init(abi_version: u32) -> u64 {
    panic::ffi_boundary(0, || {
        if !abi::is_abi_compatible(abi_version, abi::ABI_VERSION) {
            eprintln!(
                "[Morrow] ABI version mismatch: requested {abi_version:#010x}, \
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
            "[Morrow] Runtime initialized (ABI {abi_version:#010x}, handle={})",
            handle.as_u64()
        );
        handle.as_u64()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_shutdown(runtime_handle: u64) -> u32 {
    panic::ffi_boundary(abi::RESULT_ERR_PANIC, || {
        let handle = match abi::handles::Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => {
                eprintln!("[Morrow] morrow_shutdown: invalid handle 0");
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        let mut kernel = match RUNTIMES.remove(handle) {
            Some(k) => k,
            None => {
                eprintln!(
                    "[Morrow] morrow_shutdown: handle {} not found",
                    handle.as_u64()
                );
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        if let Err(state) = kernel.begin_shutdown() {
            eprintln!("[Morrow] morrow_shutdown: illegal state transition from {state}");
            RUNTIMES.insert(kernel);
            return abi::RESULT_ERR_WRONG_STATE;
        }

        // Unload all mods
        if let Some(registry) = remove_mod_registry(handle.as_u64()) {
            let count = registry.len();
            if count > 0 {
                eprintln!("[Morrow] Unloaded {count} mod(s)");
            }
            // registry drops here → libraries unloaded
        }

        if let Err(state) = kernel.finish_shutdown() {
            eprintln!("[Morrow] morrow_shutdown: finish_shutdown failed from {state}");
            return abi::RESULT_ERR_WRONG_STATE;
        }

        eprintln!("[Morrow] Runtime shut down (handle={})", handle.as_u64());
        abi::RESULT_OK
    })
}

// ---------------------------------------------------------------------------
// M3: Mod loading
// ---------------------------------------------------------------------------

/// Load a `.morrow` package into the given runtime.
///
/// # Parameters
/// - `runtime_handle`: handle from [`morrow_init`]
/// - `path_ptr`: pointer to UTF-8 path string
/// - `path_len`: length of the path string in bytes
///
/// # Returns
/// - `0` on success
/// - Non-zero error code on failure
#[unsafe(no_mangle)]
pub extern "C" fn morrow_load_mod(
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
                eprintln!("[Morrow] morrow_load_mod: invalid UTF-8 in path");
                return abi::RESULT_ERR_UNKNOWN;
            }
        };

        let package_path = Path::new(path_str);
        eprintln!("[Morrow] morrow_load_mod: {}", package_path.display());

        // Look up the mod registry for this runtime handle.
        match MOD_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
            Some(registry) => {
                let config_data = mod_loader::read_zip_config(package_path);
                match mod_loader::load_package(package_path, registry) {
                    Ok((name, exports)) => {
                        // Store config if present
                        if let Some(ref cfg) = config_data {
                            if let Some(store) = CONFIG_STORES.lock().unwrap().get(&handle.as_u64()) {
                                store.insert(&name, cfg.clone());
                                eprintln!("[Morrow]   Config loaded ({} bytes)", cfg.len());
                            }
                        }
                        if let Some(cb) = exports.tick_callback {
                            if let Some(reg) = TICK_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.register(&name, cb);
                                eprintln!("[Morrow]   Registered tick callback for '{name}'");
                            }
                        }
                        if let Some(cb) = exports.server_start_callback {
                            if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.server_start.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered server_start for '{name}'");
                            }
                        }
                        if let Some(cb) = exports.server_stop_callback {
                            if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get_mut(&handle.as_u64()) {
                                reg.server_stop.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered server_stop for '{name}'");
                            }
                        }

                        // Register event callbacks
                        if let Some(cbs) = EVENT_CALLBACKS.lock().unwrap().get_mut(&handle.as_u64()) {
                            if let Some(cb) = exports.player_join_callback {
                                cbs.player_join.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered player_join for '{name}'");
                            }
                            if let Some(cb) = exports.player_leave_callback {
                                cbs.player_leave.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered player_leave for '{name}'");
                            }
                            if let Some(cb) = exports.chat_message_callback {
                                cbs.chat_message.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered chat_message for '{name}'");
                            }
                            if let Some(cb) = exports.block_break_callback {
                                cbs.block_break.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered block_break for '{name}'");
                            }
                            if let Some(cb) = exports.block_place_callback {
                                cbs.block_place.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered block_place for '{name}'");
                            }
                            if let Some(cb) = exports.player_death_callback {
                                cbs.player_death.insert(name.clone(), cb);
                                eprintln!("[Morrow]   Registered player_death for '{name}'");
                            }
                        }

                        eprintln!("[Morrow] Mod '{name}' loaded successfully");
                        abi::RESULT_OK
                    }
                    Err(e) => {
                        eprintln!("[Morrow] Failed to load mod: {e}");
                        record_error(handle.as_u64(), format!("morrow_load_mod: {e}"));
                        abi::RESULT_ERR_UNKNOWN
                    }
                }
            }
            None => {
                eprintln!("[Morrow] morrow_load_mod: no mod registry for runtime {handle:?}");
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
pub extern "C" fn morrow_tick(runtime_handle: u64, tick_number: u64) {
    panic::ffi_boundary((), || {
        let handle = match abi::handles::Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => return,
        };

        if let Some(registry) = TICK_REGISTRIES.lock().unwrap().get(&handle.as_u64()) {
            let panicked = registry.dispatch(tick_number);
            if !panicked.is_empty() {
                // Quarantine panicking mods
                if let Some(q) = QUARANTINES.lock().unwrap().get(&handle.as_u64()) {
                    for name in &panicked {
                        q.add(name);
                        eprintln!("[Morrow] Mod '{name}' quarantined after panic");
                    }
                }
            }
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

static COMMAND_REGISTRIES: LazyLock<Mutex<HashMap<u64, host_api::CommandRegistry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static QUARANTINES: LazyLock<Mutex<HashMap<u64, host_api::Quarantine>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static CONFIG_STORES: LazyLock<Mutex<HashMap<u64, host_api::ConfigStore>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static EVENT_CALLBACKS: LazyLock<Mutex<HashMap<u64, host_api::ModEventCallbacks>>> =
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
    COMMAND_REGISTRIES.lock().unwrap().insert(handle, host_api::CommandRegistry::new());
    QUARANTINES.lock().unwrap().insert(handle, host_api::Quarantine::new());
    CONFIG_STORES.lock().unwrap().insert(handle, host_api::ConfigStore::new());
    EVENT_CALLBACKS.lock().unwrap().insert(handle, host_api::ModEventCallbacks {
        player_join: HashMap::new(),
        player_leave: HashMap::new(),
        chat_message: HashMap::new(),
        block_break: HashMap::new(),
        block_place: HashMap::new(),
        player_death: HashMap::new(),
    });
}

fn remove_mod_registry(handle: u64) -> Option<ModRegistry> {
    TICK_REGISTRIES.lock().unwrap().remove(&handle);
    LIFECYCLE_REGISTRIES.lock().unwrap().remove(&handle);
    ERROR_CHANNELS.lock().unwrap().remove(&handle);
    MOD_REGISTRIES.lock().unwrap().remove(&handle)
}

/// Register the Java host function table (upcall stubs).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_register_host_api(
    runtime_handle: u64,
    vtable_ptr: *const host_api::HostVtable,
) {
    panic::ffi_boundary((), || {
        if vtable_ptr.is_null() { return; }
        if let Some(api) = HOST_APIS.lock().unwrap().get(&runtime_handle) {
            api.set_vtable(vtable_ptr);
            eprintln!("[Morrow] Host API registered");
        }
    })
}

/// Get the online player count via Java upcall.
///
/// If `runtime_handle` is 0, uses the first available runtime.
/// Returns -1 if the host API isn't registered yet.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_player_count(runtime_handle: u64) -> i32 {
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
pub extern "C" fn morrow_handle_count() -> u64 {
    panic::ffi_boundary(0, || RUNTIMES.len() as u64)
}

/// Return the number of loaded mods across all runtimes.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_count() -> u64 {
    panic::ffi_boundary(0, || {
        MOD_REGISTRIES.lock().unwrap().values().map(|r| r.len()).sum::<usize>() as u64
    })
}

/// Return the number of quarantined mods (panicked and isolated).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_quarantined_count() -> u64 {
    panic::ffi_boundary(0, || {
        QUARANTINES.lock().unwrap().values().map(|q| q.count()).sum::<usize>() as u64
    })
}

// ---------------------------------------------------------------------------
// Host API: send_message
// ---------------------------------------------------------------------------

/// Send a chat message via Java upcall.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_send_message(
    runtime_handle: u64,
    msg_ptr: *const u8,
    msg_len: u32,
) {
    panic::ffi_boundary((), || {
        let msg = unsafe {
            let bytes = std::slice::from_raw_parts(msg_ptr, msg_len as usize);
            std::str::from_utf8(bytes).unwrap_or("<invalid utf8>")
        };
        let apis = HOST_APIS.lock().unwrap();
        let api = if runtime_handle == 0 {
            apis.values().next()
        } else {
            apis.get(&runtime_handle)
        };
        if let Some(api) = api {
            api.send_message(msg);
        }
    })
}

// ---------------------------------------------------------------------------
// Host API: get_player_list, execute_command, get_world_time
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_player_list(
    runtime_handle: u64,
    buf: *mut u8,
    buf_cap: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        let buffer = unsafe { std::slice::from_raw_parts_mut(buf, buf_cap as usize) };
        let apis = HOST_APIS.lock().unwrap();
        let api = if runtime_handle == 0 { apis.values().next() } else { apis.get(&runtime_handle) };
        api.and_then(|a| a.get_player_list(buffer)).unwrap_or(0) as u32
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_execute_command(
    runtime_handle: u64,
    cmd_ptr: *const u8,
    cmd_len: u32,
) {
    panic::ffi_boundary((), || {
        let cmd = unsafe {
            let bytes = std::slice::from_raw_parts(cmd_ptr, cmd_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        let apis = HOST_APIS.lock().unwrap();
        let api = if runtime_handle == 0 { apis.values().next() } else { apis.get(&runtime_handle) };
        if let Some(api) = api {
            api.execute_command(cmd);
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_world_time(runtime_handle: u64) -> i64 {
    panic::ffi_boundary(-1, || {
        let apis = HOST_APIS.lock().unwrap();
        let api = if runtime_handle == 0 { apis.values().next() } else { apis.get(&runtime_handle) };
        api.and_then(|a| a.get_world_time()).unwrap_or(-1)
    })
}

// ---------------------------------------------------------------------------
// Capability negotiation
// ---------------------------------------------------------------------------

/// Built-in capabilities and their versions.
static CAPABILITIES: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("event_bus", 1u32);
    m.insert("commands", 1u32);
    m.insert("host_api", 1u32);
    m.insert("config", 1u32);
    m.insert("lifecycle", 1u32);
    m.insert("player_events", 1u32);
    m.insert("block_events", 1u32);
    m.insert("panic_isolation", 1u32);
    m
});

#[unsafe(no_mangle)]
pub extern "C" fn morrow_request_capability(
    _runtime_handle: u64,
    cap_ptr: *const u8,
    cap_len: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        let cap = unsafe {
            let bytes = std::slice::from_raw_parts(cap_ptr, cap_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        CAPABILITIES.get(cap).copied().unwrap_or(0)
    })
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_mod_config(
    runtime_handle: u64,
    mod_name_ptr: *const u8, mod_name_len: u32,
    buf: *mut u8, buf_cap: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(mod_name_ptr, mod_name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        let stores = CONFIG_STORES.lock().unwrap();
        let store = if runtime_handle == 0 { stores.values().next() } else { stores.get(&runtime_handle) };
        if let Some(store) = store {
            if let Some(data) = store.get(name) {
                let len = data.len().min(buf_cap as usize);
                unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len); }
                return len as u32;
            }
        }
        0
    })
}

// ---------------------------------------------------------------------------
// Command system
// ---------------------------------------------------------------------------

/// Register a command (called by mods during init via RuntimeApi).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_register_command(
    runtime_handle: u64,
    name_ptr: *const u8,
    name_len: u32,
    callback: host_api::CommandCallback,
) {
    panic::ffi_boundary((), || {
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("<invalid>")
        };
        let registries = COMMAND_REGISTRIES.lock().unwrap();
        let reg = if runtime_handle == 0 {
            registries.values().next()
        } else {
            registries.get(&runtime_handle)
        };
        if let Some(reg) = reg {
            reg.register(name, callback);
            eprintln!("[Morrow] Command registered: /{name}");
        }
    })
}

/// Dispatch a command from Java to registered Rust callbacks.
/// Returns 1 if handled, 0 if no handler found.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_command(
    runtime_handle: u64,
    name_ptr: *const u8,
    name_len: u32,
    args_ptr: *const u8,
    args_len: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        let args = unsafe {
            let bytes = std::slice::from_raw_parts(args_ptr, args_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        if let Some(reg) = COMMAND_REGISTRIES.lock().unwrap().get(&runtime_handle) {
            if reg.dispatch(name, args) { 1 } else { 0 }
        } else {
            0
        }
    })
}

// ---------------------------------------------------------------------------
// Event dispatch: BlockBreak, BlockPlace, PlayerDeath
// ---------------------------------------------------------------------------

macro_rules! dispatch_two_str_event {
    ($name:ident, $field:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(
            runtime_handle: u64,
            s1_ptr: *const u8, s1_len: u32,
            s2_ptr: *const u8, s2_len: u32,
        ) {
            panic::ffi_boundary((), || {
                if let Some(cbs) = EVENT_CALLBACKS.lock().unwrap().get(&runtime_handle) {
                    for (_mod_name, cb) in &cbs.$field {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                            cb(s1_ptr, s1_len, s2_ptr, s2_len);
                        }));
                    }
                }
            })
        }
    };
}

dispatch_two_str_event!(morrow_dispatch_block_break, block_break);
dispatch_two_str_event!(morrow_dispatch_block_place, block_place);
dispatch_two_str_event!(morrow_dispatch_player_death, player_death);

// ---------------------------------------------------------------------------
// Event dispatch: PlayerJoin, PlayerLeave, Chat
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_player_join(
    runtime_handle: u64,
    name_ptr: *const u8,
    name_len: u32,
) {
    panic::ffi_boundary((), || {
        if let Some(cbs) = EVENT_CALLBACKS.lock().unwrap().get(&runtime_handle) {
            for (mod_name, cb) in &cbs.player_join {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                    cb(name_ptr, name_len);
                }));
            }
        }
    })
}

// Same pattern for player_leave and chat_message...
#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_player_leave(
    runtime_handle: u64,
    name_ptr: *const u8,
    name_len: u32,
) {
    panic::ffi_boundary((), || {
        if let Some(cbs) = EVENT_CALLBACKS.lock().unwrap().get(&runtime_handle) {
            for (_mod_name, cb) in &cbs.player_leave {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                    cb(name_ptr, name_len);
                }));
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_chat_message(
    runtime_handle: u64,
    player_ptr: *const u8, player_len: u32,
    msg_ptr: *const u8, msg_len: u32,
) {
    panic::ffi_boundary((), || {
        if let Some(cbs) = EVENT_CALLBACKS.lock().unwrap().get(&runtime_handle) {
            for (_mod_name, cb) in &cbs.chat_message {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                    cb(player_ptr, player_len, msg_ptr, msg_len);
                }));
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Lifecycle dispatch
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_server_start(runtime_handle: u64) {
    panic::ffi_boundary((), || {
        if let Some(reg) = LIFECYCLE_REGISTRIES.lock().unwrap().get(&runtime_handle) {
            for (name, cb) in &reg.server_start {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb() };
                }));
                if let Err(p) = result {
                    eprintln!("[Morrow] Mod '{name}' panicked in server_start: {:?}",
                        p.downcast_ref::<&str>().unwrap_or(&"<unknown>"));
                }
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_server_stop(runtime_handle: u64) {
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
pub extern "C" fn morrow_last_error(runtime_handle: u64) -> u64 {
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
pub extern "C" fn morrow_error_message(
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
