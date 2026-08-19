//! Morrow Runtime Core
//!
//! This is the Rust cdylib loaded by the Java host via Panama FFM.
//! All public symbols are `extern "C"` and use the platform C ABI.
//!
//! State model: every runtime handle maps to exactly one [`RuntimeKernel`]
//! in the global `RUNTIMES` table. All per-runtime registries live inside
//! the kernel and die with it — `morrow_shutdown` cannot leak state.

mod abi;
mod error;
mod event;
/// Host ↔ runtime ABI types ([`host_api::HostVtable`] is the vtable Java
/// registers; it is `pub` so integration tests and tooling can build one).
pub mod host_api;
mod logger;
mod mod_loader;
mod panic;
mod runtime;

use abi::handles::{Handle, HandleTable};
use event::tick::TickCallback;
use host_api::WorldSnapshot;
use runtime::RuntimeKernel;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

// ---------------------------------------------------------------------------
// Global registry of live runtime kernels, keyed by opaque handle.
// ---------------------------------------------------------------------------

static RUNTIMES: LazyLock<HandleTable<RuntimeKernel>> =
    LazyLock::new(HandleTable::new);

/// Look up a runtime kernel by its u64 handle.
///
/// A handle of `0` means "any live runtime" (the mod-facing API
/// convention — mods never know their own handle). Returns `None`
/// when no runtime matches.
fn with_runtime<F, R>(runtime_handle: u64, f: F) -> Option<R>
where
    F: FnOnce(&RuntimeKernel) -> R,
{
    if runtime_handle == 0 {
        RUNTIMES.with_first(f)
    } else {
        Handle::from_u64(runtime_handle)
            .and_then(|h| RUNTIMES.with(h, f))
    }
}

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

        // Init logger (only once)
        logger::init();

        let kernel = RuntimeKernel::new();
        let handle = RUNTIMES.insert(kernel);

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
        let handle = match Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => {
                eprintln!("[Morrow] morrow_shutdown: invalid handle 0");
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        // Removing the kernel from the table drops it — all registries
        // (mods, callbacks, config, quarantine, host API) are freed with it.
        let kernel = match RUNTIMES.remove(handle) {
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
            match std::sync::Arc::try_unwrap(kernel) {
                Ok(k) => {
                    RUNTIMES.insert(k);
                }
                Err(_) => {
                    eprintln!("[Morrow] morrow_shutdown: kernel still referenced, cannot restore");
                }
            }
            return abi::RESULT_ERR_WRONG_STATE;
        }

        // Unload all mods (drops their native libraries)
        let mod_count = kernel.data().registry.len();
        if mod_count > 0 {
            eprintln!("[Morrow] Unloaded {mod_count} mod(s)");
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
        if path_ptr.is_null() {
            eprintln!("[Morrow] morrow_load_mod: null path pointer");
            return abi::RESULT_ERR_UNKNOWN;
        }
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

        let handle = match Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => {
                eprintln!("[Morrow] morrow_load_mod: invalid runtime handle");
                return abi::RESULT_ERR_INVALID_HANDLE;
            }
        };

        // Three phases so mod code never runs under a runtime lock:
        //   A. lock held — parse manifest, check dependencies (registry read)
        //   B. no locks   — extract, dlopen, call init (mod may re-enter API)
        //   C. lock held — insert into registry, register callbacks
        RUNTIMES
            .with(handle, |kernel| {
                // ── A: parse + dependency check (data lock held) ──
                let prepared = {
                    let data = kernel.data();
                    match mod_loader::prepare_load(package_path, &data.registry) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("[Morrow] Failed to load mod: {e}");
                            data.errors.push(format!("morrow_load_mod: {e}"));
                            return abi::RESULT_ERR_UNKNOWN;
                        }
                    }
                };
                // Pure file IO — no lock needed.
                let config_data = mod_loader::read_zip_config(package_path);

                // ── B: load + init (no locks held) ──
                let (loaded, exports, name) =
                    match mod_loader::finish_load(&prepared) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("[Morrow] Failed to load mod: {e}");
                            kernel.data().errors.push(format!("morrow_load_mod: {e}"));
                            return abi::RESULT_ERR_UNKNOWN;
                        }
                    };

                // ── C: register (data lock held) ──
                let mut data = kernel.data();
                data.registry.insert(name.clone(), loaded);
                if let Some(cfg) = config_data {
                    data.configs.insert(&name, cfg);
                    eprintln!("[Morrow]   Config loaded");
                }
                let tables = {
                    if let Some(cb) = exports.server_start_callback {
                        data.lifecycle.server_start.insert(name.clone(), cb);
                        eprintln!("[Morrow]   Registered server_start for '{name}'");
                    }
                    if let Some(cb) = exports.server_stop_callback {
                        data.lifecycle.server_stop.insert(name.clone(), cb);
                        eprintln!("[Morrow]   Registered server_stop for '{name}'");
                    }
                    Arc::make_mut(&mut data.dispatch)
                };
                if let Some(cb) = exports.tick_callback {
                    tables.tick.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered tick callback for '{name}'");
                }
                if let Some(cb) = exports.player_join_callback {
                    tables.events.player_join.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered player_join for '{name}'");
                }
                if let Some(cb) = exports.player_leave_callback {
                    tables.events.player_leave.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered player_leave for '{name}'");
                }
                if let Some(cb) = exports.chat_message_callback {
                    tables.events.chat_message.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered chat_message for '{name}'");
                }
                if let Some(cb) = exports.block_break_callback {
                    tables.events.block_break.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered block_break for '{name}'");
                }
                if let Some(cb) = exports.block_place_callback {
                    tables.events.block_place.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered block_place for '{name}'");
                }
                if let Some(cb) = exports.player_death_callback {
                    tables.events.player_death.insert(name.clone(), cb);
                    eprintln!("[Morrow]   Registered player_death for '{name}'");
                }

                eprintln!("[Morrow] Mod '{name}' loaded successfully");
                abi::RESULT_OK
            })
            .unwrap_or_else(|| {
                eprintln!("[Morrow] morrow_load_mod: no runtime for handle {runtime_handle}");
                abi::RESULT_ERR_INVALID_HANDLE
            })
    })
}

// ---------------------------------------------------------------------------
// M4: Tick dispatch (legacy single-event entry, used by Java bridge tests)
// ---------------------------------------------------------------------------

/// Drive one tick cycle — dispatches to all registered mod tick callbacks.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_tick(runtime_handle: u64, tick_number: u64) {
    panic::ffi_boundary((), || {
        let handle = match Handle::from_u64(runtime_handle) {
            Some(h) => h,
            None => return,
        };

        // Snapshot the tables with one Arc bump, invoke after releasing
        // the lock (mods may re-enter the runtime API from callbacks).
        let tables = RUNTIMES.with(handle, |k| k.data().dispatch.clone());
        let Some(tables) = tables else { return };

        let panicked = run_tick_callbacks(&tables.tick, tick_number);
        quarantine_panicked(runtime_handle, &panicked);
    })
}

/// Run tick callbacks, each panic-isolated. Returns names that panicked.
fn run_tick_callbacks(callbacks: &HashMap<String, TickCallback>, tick: u64) -> Vec<String> {
    let mut panicked = Vec::new();
    for (name, cb) in callbacks {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { cb(tick) };
        }));
        if result.is_err() {
            eprintln!("[Morrow] Mod '{name}' panicked during tick {tick}");
            panicked.push(name.clone());
        }
    }
    panicked
}

/// Quarantine every mod that panicked this dispatch cycle.
fn quarantine_panicked(runtime_handle: u64, panicked: &[String]) {
    if panicked.is_empty() {
        return;
    }
    with_runtime(runtime_handle, |kernel| {
        let mut data = kernel.data();
        for name in panicked {
            Arc::make_mut(&mut data.dispatch).quarantined.insert(name.clone());
            eprintln!("[Morrow] Mod '{name}' quarantined after panic");
        }
    });
}

// ---------------------------------------------------------------------------
// Batch event dispatch (1 FFM call/tick) — the production path
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_batch(
    runtime_handle: u64,
    data_ptr: *const u8,
    data_len: u32,
) {
    panic::ffi_boundary((), || {
        if data_ptr.is_null() || data_len < 4 {
            return;
        }
        let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize) };

        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut pos: usize = 4;

        // Snapshot the dispatch tables with one Arc bump, then run
        // callbacks with no runtime lock held — mods may re-enter the
        // API from callbacks without deadlocking.
        let dispatch = with_runtime(runtime_handle, |kernel| {
            let data = kernel.data();
            (
                data.dispatch.clone(),
                data.host_api.clone(),
                data.snapshot_consumers > 0,
            )
        });
        let (tables, host_api, snapshot_wanted) = match dispatch {
            Some(d) => d,
            None => return,
        };

        // dispatch_batch always runs on the game main thread: record it
        // (so off-thread mod writes queue instead of touching the game)
        // and deliver anything mod threads queued since the last tick.
        host_api.note_main_thread();
        host_api.flush_outbound();

        // Refresh the world snapshot once per tick (1 upcall, not N) —
        // but only while a mod-facing API actually consumes it. v0.16
        // ships no snapshot query API, so this skips the upcall and its
        // O(players) serialization on the Java game thread entirely.
        if snapshot_wanted {
            let mut snap_buf = [0u8; 4096];
            let snapshot = host_api
                .get_world_snapshot(&mut snap_buf)
                .and_then(|n| WorldSnapshot::parse(&snap_buf[..n]));
            if let Some(snap) = snapshot {
                with_runtime(runtime_handle, |kernel| {
                    kernel.data().snapshot = Some(snap);
                });
            }
        }

        let mut panicked: Vec<String> = Vec::new();

        for _ in 0..count {
            if pos + 6 > data.len() {
                break;
            }
            let etype = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            let f1_len = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
            let f2_len = u16::from_le_bytes([data[pos + 4], data[pos + 5]]) as usize;
            pos += 6;

            match etype {
                0 => {
                    // tick
                    if pos + 8 <= data.len() {
                        let tick = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                        pos += 8;
                        for (name, cb) in &tables.tick {
                            if tables.quarantined.contains(name) {
                                continue;
                            }
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                cb(tick);
                            }))
                            .is_err()
                            {
                                eprintln!("[Morrow] Mod '{name}' panicked during tick {tick}");
                                panicked.push(name.clone());
                            }
                        }
                    }
                }
                1 | 2 => {
                    // join / leave
                    if pos + f1_len <= data.len() {
                        pos += f1_len;
                        let map = if etype == 1 {
                            &tables.events.player_join
                        } else {
                            &tables.events.player_leave
                        };
                        for (name, cb) in map {
                            if tables.quarantined.contains(name) {
                                continue;
                            }
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                cb(data[pos - f1_len..pos].as_ptr(), f1_len as u32);
                            }))
                            .is_err()
                            {
                                panicked.push(name.clone());
                            }
                        }
                    }
                }
                3 => {
                    // chat
                    if pos + f1_len + f2_len <= data.len() {
                        let p_ptr = data[pos..pos + f1_len].as_ptr();
                        let m_ptr = data[pos + f1_len..pos + f1_len + f2_len].as_ptr();
                        pos += f1_len + f2_len;
                        for (name, cb) in &tables.events.chat_message {
                            if tables.quarantined.contains(name) {
                                continue;
                            }
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                cb(p_ptr, f1_len as u32, m_ptr, f2_len as u32);
                            }))
                            .is_err()
                            {
                                panicked.push(name.clone());
                            }
                        }
                    }
                }
                4 | 5 => {
                    // block break/place
                    if pos + f1_len + f2_len <= data.len() {
                        let p_ptr = data[pos..pos + f1_len].as_ptr();
                        let b_ptr = data[pos + f1_len..pos + f1_len + f2_len].as_ptr();
                        pos += f1_len + f2_len;
                        let map = if etype == 4 {
                            &tables.events.block_break
                        } else {
                            &tables.events.block_place
                        };
                        for (name, cb) in map {
                            if tables.quarantined.contains(name) {
                                continue;
                            }
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                cb(p_ptr, f1_len as u32, b_ptr, f2_len as u32);
                            }))
                            .is_err()
                            {
                                panicked.push(name.clone());
                            }
                        }
                    }
                }
                6 => {
                    // player death
                    if pos + f1_len <= data.len() {
                        pos += f1_len;
                        for (name, cb) in &tables.events.player_death {
                            if tables.quarantined.contains(name) {
                                continue;
                            }
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                cb(
                                    data[pos - f1_len..pos].as_ptr(),
                                    f1_len as u32,
                                    std::ptr::null(),
                                    0u32,
                                );
                            }))
                            .is_err()
                            {
                                panicked.push(name.clone());
                            }
                        }
                    }
                }
                _ => {
                    pos += f1_len + f2_len; // skip unknown
                }
            }
        }

        // Quarantine every mod that panicked — batch path now matches
        // the legacy single-event path.
        quarantine_panicked(runtime_handle, &panicked);
    })
}

// ---------------------------------------------------------------------------
// Host API (mod → Java upcalls)
// ---------------------------------------------------------------------------

/// Get the online player count via Java upcall.
///
/// If `runtime_handle` is 0, uses the first available runtime.
/// Returns -1 if the host API isn't registered yet.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_player_count(runtime_handle: u64) -> i32 {
    panic::ffi_boundary(-1, || {
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.get_player_count()
        })
        .flatten()
        .unwrap_or(-1)
    })
}

/// Send a chat message via Java upcall.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_send_message(
    runtime_handle: u64,
    msg_ptr: *const u8,
    msg_len: u32,
) {
    panic::ffi_boundary((), || {
        if msg_ptr.is_null() {
            return;
        }
        let msg = unsafe {
            let bytes = std::slice::from_raw_parts(msg_ptr, msg_len as usize);
            std::str::from_utf8(bytes).unwrap_or("<invalid utf8>")
        };
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.send_message(msg)
        });
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_player_list(
    runtime_handle: u64,
    buf: *mut u8,
    buf_cap: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        if buf.is_null() {
            return 0;
        }
        let buffer = unsafe { std::slice::from_raw_parts_mut(buf, buf_cap as usize) };
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.get_player_list(buffer)
        })
        .flatten()
        .unwrap_or(0) as u32
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_execute_command(
    runtime_handle: u64,
    cmd_ptr: *const u8,
    cmd_len: u32,
) {
    panic::ffi_boundary((), || {
        if cmd_ptr.is_null() {
            return;
        }
        let cmd = unsafe {
            let bytes = std::slice::from_raw_parts(cmd_ptr, cmd_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.execute_command(cmd)
        });
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_world_time(runtime_handle: u64) -> i64 {
    panic::ffi_boundary(-1, || {
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.get_world_time()
        })
        .flatten()
        .unwrap_or(-1)
    })
}

// ---------------------------------------------------------------------------
// Capability negotiation
// ---------------------------------------------------------------------------

/// Built-in capabilities and their versions.
static CAPABILITIES: LazyLock<std::collections::HashMap<&'static str, u32>> =
    LazyLock::new(|| {
        let mut m = std::collections::HashMap::new();
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
        if cap_ptr.is_null() {
            return 0;
        }
        let cap = unsafe {
            let bytes = std::slice::from_raw_parts(cap_ptr, cap_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        CAPABILITIES.get(cap).copied().unwrap_or(0)
    })
}

// ---------------------------------------------------------------------------
// Mod logging
// ---------------------------------------------------------------------------

/// Level prefixes for `morrow_mod_log` (1=info, 2=warn, 3=error).
fn level_prefix(level: u32) -> &'static str {
    match level {
        3 => "ERROR",
        2 => "WARN",
        _ => "INFO",
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_log(
    runtime_handle: u64,
    level: u32,
    msg_ptr: *const u8,
    msg_len: u32,
) {
    panic::ffi_boundary((), || {
        if msg_ptr.is_null() {
            return;
        }
        let msg = unsafe {
            let bytes = std::slice::from_raw_parts(msg_ptr, msg_len as usize);
            std::str::from_utf8(bytes).unwrap_or("<invalid utf8>")
        };
        eprintln!("[Morrow:{}] {}", level_prefix(level), msg);
        // Also forward to Java with the level intact
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.log_message(level, msg)
        });
    })
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_get_mod_config(
    runtime_handle: u64,
    mod_name_ptr: *const u8,
    mod_name_len: u32,
    buf: *mut u8,
    buf_cap: u32,
) -> u32 {
    panic::ffi_boundary(0, || {
        if mod_name_ptr.is_null() || buf.is_null() {
            return 0;
        }
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(mod_name_ptr, mod_name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        with_runtime(runtime_handle, |kernel| {
            let store = kernel.data();
            if let Some(data) = store.configs.get(name) {
                let len = data.len().min(buf_cap as usize);
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len);
                }
                len as u32
            } else {
                0
            }
        })
        .unwrap_or(0)
    })
}

// ---------------------------------------------------------------------------
// Command system
// ---------------------------------------------------------------------------

/// Register a command (called by mods during init via RuntimeApi).
/// Returns 0 on success, 1 if the name is already taken (conflicting
/// registration is a config error — surfaced to the mod, not overwritten).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_register_command(
    runtime_handle: u64,
    name_ptr: *const u8,
    name_len: u32,
    callback: host_api::CommandCallback,
) -> u32 {
    panic::ffi_boundary(1, || {
        if name_ptr.is_null() {
            return 1;
        }
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("<invalid>")
        };
        let result = with_runtime(runtime_handle, |kernel| {
            kernel.data().commands.register(name, callback)
        });
        match result {
            Some(Ok(())) => {
                eprintln!("[Morrow] Command registered: /{name}");
                0
            }
            Some(Err(e)) => {
                eprintln!("[Morrow] ERROR: {e}");
                1
            }
            None => {
                eprintln!("[Morrow] ERROR: no live runtime (handle={runtime_handle})");
                1
            }
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
        if name_ptr.is_null() {
            return 0;
        }
        let name = unsafe {
            let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        let args = unsafe {
            let bytes = std::slice::from_raw_parts(args_ptr, args_len as usize);
            std::str::from_utf8(bytes).unwrap_or("")
        };
        // Snapshot the callback under the runtime lock, then run it
        // outside — the handler may re-enter the API (send_message,
        // execute_command, register_command, ...), each of which takes
        // the runtime data lock again.
        let handled = with_runtime(runtime_handle, |kernel| {
            kernel.data().commands.lookup(name)
        })
        .flatten()
        .map(|cb| {
            let b = args.as_bytes();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                unsafe { cb(b.as_ptr(), b.len() as u32); }
            }));
            true
        })
        .unwrap_or(false);
        if handled { 1 } else { 0 }
    })
}

// ---------------------------------------------------------------------------
// Lifecycle dispatch
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_server_start(runtime_handle: u64) {
    panic::ffi_boundary((), || {
        with_runtime(runtime_handle, |kernel| {
            let callbacks: Vec<(String, unsafe extern "C" fn())> =
                kernel.data().lifecycle.server_start.clone().into_iter().collect();
            for (name, cb) in &callbacks {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb() };
                }));
                if let Err(p) = result {
                    eprintln!(
                        "[Morrow] Mod '{name}' panicked in server_start: {:?}",
                        p.downcast_ref::<&str>().unwrap_or(&"<unknown>")
                    );
                }
            }
        });
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_dispatch_server_stop(runtime_handle: u64) {
    panic::ffi_boundary((), || {
        with_runtime(runtime_handle, |kernel| {
            let callbacks: Vec<(String, unsafe extern "C" fn())> =
                kernel.data().lifecycle.server_stop.clone().into_iter().collect();
            for (_name, cb) in &callbacks {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb() };
                }));
            }
        });
    })
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
        RUNTIMES.fold(0u64, |acc, kernel| acc + kernel.data().registry.len() as u64)
    })
}

/// Return the number of quarantined mods (panicked and isolated).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_quarantined_count() -> u64 {
    panic::ffi_boundary(0, || {
        RUNTIMES.fold(0u64, |acc, kernel| acc + kernel.data().dispatch.quarantined.len() as u64)
    })
}

// ---------------------------------------------------------------------------
// Host API registration (Java → Rust vtable)
// ---------------------------------------------------------------------------

/// Register the Java host function table (upcall stubs).
#[unsafe(no_mangle)]
pub extern "C" fn morrow_register_host_api(
    runtime_handle: u64,
    vtable_ptr: *const host_api::HostVtable,
) {
    panic::ffi_boundary((), || {
        if vtable_ptr.is_null() {
            return;
        }
        with_runtime(runtime_handle, |kernel| {
            kernel.data().host_api.set_vtable(vtable_ptr);
        });
        eprintln!("[Morrow] Host API registered");
    })
}

// ---------------------------------------------------------------------------
// Error Channel
// ---------------------------------------------------------------------------

/// Get the handle of the oldest pending error, or 0 if no errors.
#[unsafe(no_mangle)]
pub extern "C" fn morrow_last_error(runtime_handle: u64) -> u64 {
    panic::ffi_boundary(0, || {
        with_runtime(runtime_handle, |kernel| {
            kernel
                .data()
                .errors
                .peek()
                .map(|e| e.id)
                .unwrap_or(0)
        })
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
        if buffer_ptr.is_null() {
            return 0;
        }
        with_runtime(runtime_handle, |kernel| {
            let record = match kernel.data().errors.take(error_handle) {
                Some(r) => r,
                None => return 0,
            };

            let bytes = record.message.as_bytes();
            let len = bytes.len().min(buffer_cap as usize);

            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer_ptr, len);
            }

            len as u32
        })
        .unwrap_or(0)
    })
}
