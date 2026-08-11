//! Host API — function pointers from Java + RuntimeApi for mods.
//!
//! Two-way bridge:
//! - **HostVtable**: Java→Rust upcall stubs (get_player_count, send_message)
//! - **RuntimeApi**: Rust→mod function table (passed to ferrum_mod_init)

use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Runtime API — passed to mods during init
// ---------------------------------------------------------------------------

/// Function table passed to `ferrum_mod_init(api: *const RuntimeApi)`.
#[repr(C)]
pub struct RuntimeApi {
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
    pub send_message: unsafe extern "C" fn(runtime_handle: u64, msg_ptr: *const u8, msg_len: u32),
    pub register_command: unsafe extern "C" fn(
        runtime_handle: u64,
        name_ptr: *const u8, name_len: u32,
        callback: unsafe extern "C" fn(*const u8, u32),
    ),
}

impl RuntimeApi {
    pub fn new() -> Self {
        RuntimeApi {
            get_player_count: crate::ferrum_get_player_count,
            send_message: crate::ferrum_send_message,
            register_command: crate::ferrum_register_command,
        }
    }
}

// ---------------------------------------------------------------------------
// Host Vtable (from Java)
// ---------------------------------------------------------------------------

type GetPlayerCountFn = unsafe extern "C" fn() -> i32;
type SendMessageFn = unsafe extern "C" fn(*const u8, u32);

#[repr(C)]
pub struct HostVtable {
    pub get_player_count: Option<GetPlayerCountFn>,
    pub send_message: Option<SendMessageFn>,
}

pub struct HostApi {
    vtable: Mutex<Option<HostVtable>>,
}

impl HostApi {
    pub fn new() -> Self {
        HostApi { vtable: Mutex::new(None) }
    }

    pub fn set_vtable(&self, ptr: *const HostVtable) {
        unsafe { *self.vtable.lock().unwrap() = Some(ptr.read()); }
    }

    pub fn get_player_count(&self) -> Option<i32> {
        let guard = self.vtable.lock().unwrap();
        let func = guard.as_ref()?.get_player_count?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { func() }));
        result.ok()
    }

    pub fn send_message(&self, msg: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.send_message else { return false };
        let bytes = msg.as_bytes();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(bytes.as_ptr(), bytes.len() as u32)
        }));
        result.is_ok()
    }
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

pub type CommandCallback = unsafe extern "C" fn(args_ptr: *const u8, args_len: u32);

pub struct CommandRegistry {
    commands: Mutex<HashMap<String, CommandCallback>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        CommandRegistry { commands: Mutex::new(HashMap::new()) }
    }

    pub fn register(&self, name: &str, callback: CommandCallback) {
        self.commands.lock().unwrap().insert(name.to_string(), callback);
    }

    pub fn dispatch(&self, name: &str, args: &str) -> bool {
        let guard = self.commands.lock().unwrap();
        if let Some(cb) = guard.get(name) {
            let args_bytes = args.as_bytes();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                cb(args_bytes.as_ptr(), args_bytes.len() as u32);
            }));
            true
        } else {
            false
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Per-mod callbacks (for events)
// ---------------------------------------------------------------------------

pub type PlayerEventCallback = unsafe extern "C" fn(*const u8, u32);
pub type ChatEventCallback = unsafe extern "C" fn(*const u8, u32, *const u8, u32);

pub struct ModEventCallbacks {
    pub player_join: HashMap<String, PlayerEventCallback>,
    pub player_leave: HashMap<String, PlayerEventCallback>,
    pub chat_message: HashMap<String, ChatEventCallback>,
}
