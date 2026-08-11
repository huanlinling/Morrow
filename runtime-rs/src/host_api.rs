//! Host API — two-way bridge between Rust mods and Java game server.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Runtime API — passed to mods during init
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct RuntimeApi {
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
    pub send_message: unsafe extern "C" fn(runtime_handle: u64, msg_ptr: *const u8, msg_len: u32),
    pub register_command: unsafe extern "C" fn(
        runtime_handle: u64,
        name_ptr: *const u8, name_len: u32,
        callback: unsafe extern "C" fn(*const u8, u32),
    ),
    pub get_player_list: unsafe extern "C" fn(runtime_handle: u64, buf: *mut u8, buf_cap: u32) -> u32,
    pub execute_command: unsafe extern "C" fn(runtime_handle: u64, cmd_ptr: *const u8, cmd_len: u32),
    pub get_world_time: unsafe extern "C" fn(runtime_handle: u64) -> i64,
    /// Log a message through the runtime's logger. level: 1=info, 2=warn, 3=error.
    pub log: unsafe extern "C" fn(runtime_handle: u64, level: u32, msg_ptr: *const u8, msg_len: u32),
    /// Read the mod's config.toml. Returns bytes written, 0 if no config.
    pub get_config: unsafe extern "C" fn(runtime_handle: u64, mod_name_ptr: *const u8, mod_name_len: u32, buf: *mut u8, buf_cap: u32) -> u32,
    /// Request a capability version. Returns version (1, 2, ...) or 0 if unavailable.
    pub request_capability: unsafe extern "C" fn(runtime_handle: u64, cap_ptr: *const u8, cap_len: u32) -> u32,
}

impl RuntimeApi {
    pub fn new() -> Self {
        RuntimeApi {
            get_player_count: crate::morrow_get_player_count,
            send_message: crate::morrow_send_message,
            register_command: crate::morrow_register_command,
            get_player_list: crate::morrow_get_player_list,
            execute_command: crate::morrow_execute_command,
            get_world_time: crate::morrow_get_world_time,
            log: crate::morrow_mod_log,
            get_config: crate::morrow_get_mod_config,
            request_capability: crate::morrow_request_capability,
        }
    }
}

// ---------------------------------------------------------------------------
// Mod config storage
// ---------------------------------------------------------------------------

pub struct ConfigStore {
    configs: Mutex<HashMap<String, Vec<u8>>>,
}

impl ConfigStore {
    pub fn new() -> Self { ConfigStore { configs: Mutex::new(HashMap::new()) } }
    pub fn insert(&self, name: &str, data: Vec<u8>) {
        self.configs.lock().unwrap().insert(name.to_string(), data);
    }
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        self.configs.lock().unwrap().get(name).cloned()
    }
}

impl Default for ConfigStore { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Host Vtable (from Java)
// ---------------------------------------------------------------------------

type GetPlayerCountFn = unsafe extern "C" fn() -> i32;
type SendMessageFn = unsafe extern "C" fn(*const u8, u32);
type GetPlayerListFn = unsafe extern "C" fn(*mut u8, u32) -> u32;
type ExecuteCommandFn = unsafe extern "C" fn(*const u8, u32);
type GetWorldTimeFn = unsafe extern "C" fn() -> i64;
type LogMessageFn = unsafe extern "C" fn(u32, *const u8, u32);

#[repr(C)]
pub struct HostVtable {
    pub get_player_count: Option<GetPlayerCountFn>,
    pub send_message: Option<SendMessageFn>,
    pub get_player_list: Option<GetPlayerListFn>,
    pub execute_command: Option<ExecuteCommandFn>,
    pub get_world_time: Option<GetWorldTimeFn>,
    pub log_message: Option<LogMessageFn>,
}

pub struct HostApi {
    vtable: Mutex<Option<HostVtable>>,
}

impl HostApi {
    pub fn new() -> Self { HostApi { vtable: Mutex::new(None) } }

    pub fn set_vtable(&self, ptr: *const HostVtable) {
        unsafe { *self.vtable.lock().unwrap() = Some(ptr.read()); }
    }

    pub fn get_player_count(&self) -> Option<i32> {
        let guard = self.vtable.lock().unwrap();
        let func = guard.as_ref()?.get_player_count?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { func() })).ok()
    }

    pub fn send_message(&self, msg: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.send_message else { return false };
        let bytes = msg.as_bytes();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(bytes.as_ptr(), bytes.len() as u32)
        })).is_ok()
    }

    pub fn get_player_list(&self, buf: &mut [u8]) -> Option<usize> {
        let guard = self.vtable.lock().unwrap();
        let func = guard.as_ref()?.get_player_list?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(buf.as_mut_ptr(), buf.len() as u32) as usize
        }));
        result.ok().map(|n| n.min(buf.len()))
    }

    pub fn execute_command(&self, cmd: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.execute_command else { return false };
        let bytes = cmd.as_bytes();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(bytes.as_ptr(), bytes.len() as u32)
        })).is_ok()
    }

    pub fn get_world_time(&self) -> Option<i64> {
        let guard = self.vtable.lock().unwrap();
        let func = guard.as_ref()?.get_world_time?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { func() })).ok()
    }

    pub fn log_message(&self, level: u32, msg: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.log_message else { return false };
        let bytes = msg.as_bytes();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(level, bytes.as_ptr(), bytes.len() as u32);
        })).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Command / Event registries (unchanged)
// ---------------------------------------------------------------------------

pub type CommandCallback = unsafe extern "C" fn(args_ptr: *const u8, args_len: u32);

pub struct CommandRegistry {
    commands: Mutex<HashMap<String, CommandCallback>>,
}

impl CommandRegistry {
    pub fn new() -> Self { CommandRegistry { commands: Mutex::new(HashMap::new()) } }
    pub fn register(&self, name: &str, callback: CommandCallback) {
        self.commands.lock().unwrap().insert(name.to_string(), callback);
    }
    pub fn dispatch(&self, name: &str, args: &str) -> bool {
        let guard = self.commands.lock().unwrap();
        if let Some(cb) = guard.get(name) {
            let b = args.as_bytes();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { cb(b.as_ptr(), b.len() as u32); }));
            true
        } else { false }
    }
}

impl Default for CommandRegistry { fn default() -> Self { Self::new() } }

pub type PlayerEventCallback = unsafe extern "C" fn(*const u8, u32);
pub type TwoStrEventCallback = unsafe extern "C" fn(*const u8, u32, *const u8, u32);

pub struct ModEventCallbacks {
    pub player_join: HashMap<String, PlayerEventCallback>,
    pub player_leave: HashMap<String, PlayerEventCallback>,
    pub chat_message: HashMap<String, TwoStrEventCallback>,
    pub block_break: HashMap<String, TwoStrEventCallback>,
    pub block_place: HashMap<String, TwoStrEventCallback>,
    pub player_death: HashMap<String, TwoStrEventCallback>,
}

// ---------------------------------------------------------------------------
// Panic quarantine
// ---------------------------------------------------------------------------

pub struct Quarantine {
    pub(crate) quarantined: Mutex<HashSet<String>>,
}

impl Quarantine {
    pub fn new() -> Self { Quarantine { quarantined: Mutex::new(HashSet::new()) } }
    pub fn add(&self, name: &str) { self.quarantined.lock().unwrap().insert(name.to_string()); }
    pub fn is_quarantined(&self, name: &str) -> bool { self.quarantined.lock().unwrap().contains(name) }
    pub fn count(&self) -> usize { self.quarantined.lock().unwrap().len() }
}

impl Default for Quarantine { fn default() -> Self { Self::new() } }
