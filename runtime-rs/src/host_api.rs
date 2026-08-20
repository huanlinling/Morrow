//! Host API — two-way bridge between Rust mods and Java game server.

use crate::event::tick::TickCallback;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

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
    ) -> u32,
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
type GetWorldSnapshotFn = unsafe extern "C" fn(*mut u8, u32) -> u32;

#[repr(C)]
#[derive(Clone)]
pub struct HostVtable {
    pub get_player_count: Option<GetPlayerCountFn>,
    pub send_message: Option<SendMessageFn>,
    pub get_player_list: Option<GetPlayerListFn>,
    pub execute_command: Option<ExecuteCommandFn>,
    pub get_world_time: Option<GetWorldTimeFn>,
    pub log_message: Option<LogMessageFn>,
    pub get_world_snapshot: Option<GetWorldSnapshotFn>,
}

/// A game-state write requested from a non-main thread. Minecraft is
/// single-threaded: touching game state off the main thread races the
/// server and can crash the JVM. Queued instead, delivered by
/// [`HostApi::flush_outbound`] on the main thread at the next tick.
enum Outbound {
    Message(String),
    Command(String),
}

/// State shared across `HostApi` clones (one clone is taken per tick):
/// the recorded main-thread identity and the cross-thread outbox.
/// Without `Arc`, queued writes would strand in the per-tick clone.
struct HostApiShared {
    main_thread: Mutex<Option<ThreadId>>,
    outbound: Mutex<Vec<Outbound>>,
}

pub struct HostApi {
    vtable: Mutex<Option<HostVtable>>,
    shared: Arc<HostApiShared>,
}

impl Clone for HostApi {
    fn clone(&self) -> Self {
        HostApi {
            vtable: Mutex::new(self.vtable.lock().unwrap().clone()),
            shared: self.shared.clone(),
        }
    }
}

impl HostApi {
    pub fn new() -> Self {
        HostApi {
            vtable: Mutex::new(None),
            shared: Arc::new(HostApiShared {
                main_thread: Mutex::new(None),
                outbound: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn set_vtable(&self, ptr: *const HostVtable) {
        unsafe { *self.vtable.lock().unwrap() = Some(ptr.read()); }
    }

    /// Record the calling thread as the game main thread. Called from
    /// `morrow_dispatch_batch`, which always runs on the main thread.
    pub fn note_main_thread(&self) {
        *self.shared.main_thread.lock().unwrap() = Some(std::thread::current().id());
    }

    /// True when called on the recorded game main thread. Before the
    /// first dispatch records one, everything is treated as off-main:
    /// writes queue and are delivered at the first tick.
    fn on_main_thread(&self) -> bool {
        self.shared
            .main_thread
            .lock()
            .unwrap()
            .is_some_and(|id| id == std::thread::current().id())
    }

    /// Deliver queued writes. Must run on the game main thread (called
    /// from `morrow_dispatch_batch` before event dispatch).
    pub fn flush_outbound(&self) {
        let pending = std::mem::take(&mut *self.shared.outbound.lock().unwrap());
        for item in pending {
            match item {
                Outbound::Message(msg) => { self.send_message_direct(&msg); }
                Outbound::Command(cmd) => { self.execute_command_direct(&cmd); }
            }
        }
    }

    /// Broadcast a chat message. Safe from any thread: on the game main
    /// thread it goes out immediately; from any other thread it is
    /// queued and delivered at the next tick (≤ 50 ms later).
    pub fn send_message(&self, msg: &str) -> bool {
        if self.on_main_thread() {
            self.send_message_direct(msg)
        } else {
            self.shared
                .outbound
                .lock()
                .unwrap()
                .push(Outbound::Message(msg.to_string()));
            true
        }
    }

    fn send_message_direct(&self, msg: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.send_message else { return false };
        let bytes = msg.as_bytes();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(bytes.as_ptr(), bytes.len() as u32)
        })).is_ok()
    }

    /// Execute a console command. Same threading contract as
    /// [`HostApi::send_message`]: immediate on the main thread, queued
    /// to the next tick from any other thread.
    pub fn execute_command(&self, cmd: &str) -> bool {
        if self.on_main_thread() {
            self.execute_command_direct(cmd)
        } else {
            self.shared
                .outbound
                .lock()
                .unwrap()
                .push(Outbound::Command(cmd.to_string()));
            true
        }
    }

    fn execute_command_direct(&self, cmd: &str) -> bool {
        let guard = self.vtable.lock().unwrap();
        let Some(vtable) = guard.as_ref() else { return false };
        let Some(func) = vtable.execute_command else { return false };
        let bytes = cmd.as_bytes();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(bytes.as_ptr(), bytes.len() as u32)
        })).is_ok()
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

    pub fn get_world_snapshot(&self, buf: &mut [u8]) -> Option<usize> {
        let guard = self.vtable.lock().unwrap();
        let func = guard.as_ref()?.get_world_snapshot?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            func(buf.as_mut_ptr(), buf.len() as u32) as usize
        }));
        result.ok().map(|n| n.min(buf.len()))
    }
}

// ---------------------------------------------------------------------------
// World snapshot
// ---------------------------------------------------------------------------

/// Cached world state, refreshed once per tick via a single upcall.
// Fields read by mod-facing APIs in later milestones.
#[derive(Clone)]
#[allow(dead_code)]
pub struct WorldSnapshot {
    pub player_count: u32,
    pub world_time: i64,
    /// Player names (borrowed from snapshot buffer).
    pub player_names: Vec<String>,
}

impl WorldSnapshot {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 { return None; }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let time = i64::from_le_bytes(data[4..12].try_into().unwrap());
        let mut names = Vec::with_capacity(count as usize);
        let mut pos = 12;
        for _ in 0..count {
            if pos + 2 > data.len() { break; }
            let len = u16::from_le_bytes([data[pos], data[pos+1]]) as usize; pos += 2;
            if pos + len > data.len() { break; }
            names.push(String::from_utf8_lossy(&data[pos..pos+len]).into_owned());
            pos += len;
        }
        Some(WorldSnapshot { player_count: count, world_time: time, player_names: names })
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
    /// Register a command. Rejects duplicate names — a second mod (or the
    /// same mod) claiming the same command is a config error that must be
    /// surfaced, not silently overwritten.
    pub fn register(&self, name: &str, callback: CommandCallback) -> Result<(), String> {
        let mut guard = self.commands.lock().unwrap();
        if guard.contains_key(name) {
            return Err(format!("command '/{name}' already registered by another mod"));
        }
        guard.insert(name.to_string(), callback);
        Ok(())
    }
    /// Snapshot the callback for `name`, if registered (function pointers
    /// are `Copy`). Never call the callback from a context that holds the
    /// runtime's data lock — handlers re-enter the API.
    pub fn lookup(&self, name: &str) -> Option<CommandCallback> {
        self.commands.lock().unwrap().get(name).copied()
    }

    /// Dispatch `args` to the handler for `name`.
    ///
    /// The callback is snapshotted under the registry lock and invoked
    /// with no locks held — handlers may re-enter the runtime API
    /// (send_message, execute_command, register_command, ...) which
    /// itself takes the runtime's data lock. Callers must not hold that
    /// data lock across this call.
    pub fn dispatch(&self, name: &str, args: &str) -> bool {
        match self.lookup(name) {
            Some(cb) => {
                let b = args.as_bytes();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { cb(b.as_ptr(), b.len() as u32); }
                }));
                true
            }
            None => false,
        }
    }
}

impl Default for CommandRegistry { fn default() -> Self { Self::new() } }

pub type PlayerEventCallback = unsafe extern "C" fn(*const u8, u32);
pub type TwoStrEventCallback = unsafe extern "C" fn(*const u8, u32, *const u8, u32);

#[derive(Clone)]
pub struct ModEventCallbacks {
    pub player_join: HashMap<String, PlayerEventCallback>,
    pub player_leave: HashMap<String, PlayerEventCallback>,
    pub chat_message: HashMap<String, TwoStrEventCallback>,
    pub block_break: HashMap<String, TwoStrEventCallback>,
    pub block_place: HashMap<String, TwoStrEventCallback>,
    pub player_death: HashMap<String, TwoStrEventCallback>,
}

impl ModEventCallbacks {
    pub fn new() -> Self {
        ModEventCallbacks {
            player_join: HashMap::new(),
            player_leave: HashMap::new(),
            chat_message: HashMap::new(),
            block_break: HashMap::new(),
            block_place: HashMap::new(),
            player_death: HashMap::new(),
        }
    }
}

impl Default for ModEventCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dispatch tables — one Arc snapshot per tick instead of N map clones
// ---------------------------------------------------------------------------

/// Everything the per-tick dispatch loop reads, behind a single `Arc`.
///
/// `morrow_dispatch_batch` snapshots the tables with one refcount bump
/// (`Arc::clone`); the old code cloned every HashMap/HashSet on every
/// tick (~9 allocations). Mutations (mod registration, quarantine) are
/// rare and go through `Arc::make_mut` — clone-on-write there is fine.
#[derive(Clone, Default)]
pub struct DispatchTables {
    pub tick: HashMap<String, TickCallback>,
    pub events: ModEventCallbacks,
    pub quarantined: HashSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    static SENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn rec_send(ptr: *const u8, len: u32) {
        let s = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        SENT.lock().unwrap().push(String::from_utf8(s.to_vec()).unwrap());
    }

    #[test]
    fn off_main_writes_queue_and_flush_on_main() {
        let api = HostApi::new();
        api.set_vtable(&HostVtable {
            get_player_count: None,
            send_message: Some(rec_send),
            get_player_list: None,
            execute_command: None,
            get_world_time: None,
            log_message: None,
            get_world_snapshot: None,
        });

        // Before any main thread is known, even this thread queues.
        api.send_message("early");
        assert!(SENT.lock().unwrap().is_empty());

        // A mod-spawned thread must not touch the host directly.
        let api2 = api.clone();
        std::thread::spawn(move || {
            assert!(api2.send_message("hi"));
        })
        .join()
        .unwrap();
        assert!(SENT.lock().unwrap().is_empty(), "off-main write must queue");

        // Next tick on the main thread: record + flush, FIFO order.
        api.note_main_thread();
        api.flush_outbound();
        assert_eq!(SENT.lock().unwrap().as_slice(), ["early", "hi"]);

        // Main-thread calls now go direct, nothing queued.
        api.send_message("now");
        assert_eq!(SENT.lock().unwrap().as_slice(), ["early", "hi", "now"]);
        assert!(api.shared.outbound.lock().unwrap().is_empty());
    }
}
