//! Black-box integration test: real `.morrow` package → real loader →
//! real event dispatch. No Minecraft involved — the runtime's exported
//! C ABI is driven through the same entry points the Java host uses.
//!
//! The host vtable (normally upcall stubs from Java) is built with the
//! real `runtime_rs::host_api::HostVtable` type, so the full
//! mod → runtime → host loop is exercised.

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use morrow_runtime::host_api::HostVtable;

const ABI_VERSION: u32 = 0x0001_0000;

// ---------------------------------------------------------------------------
// Test host vtable — same shape Java builds with upcall stubs
// ---------------------------------------------------------------------------

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static SENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

unsafe extern "C" fn test_get_player_count() -> i32 {
    7
}

unsafe extern "C" fn test_send_message(ptr: *const u8, len: u32) {
    SENT.lock().unwrap().push(read_str(ptr, len).to_string());
}

unsafe extern "C" fn test_get_player_list(_buf: *mut u8, _cap: u32) -> u32 {
    0
}

unsafe extern "C" fn test_execute_command(_ptr: *const u8, _len: u32) {}

unsafe extern "C" fn test_get_world_time() -> i64 {
    12345
}

unsafe extern "C" fn test_log(level: u32, ptr: *const u8, len: u32) {
    LOG.lock()
        .unwrap()
        .push(format!("L{level}:{}", read_str(ptr, len)));
}

unsafe extern "C" fn test_get_world_snapshot(_buf: *mut u8, _cap: u32) -> u32 {
    0
}

fn read_str<'a>(ptr: *const u8, len: u32) -> &'a str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(ptr, len as usize)).unwrap_or("<bad utf-8>")
    }
}

// ---------------------------------------------------------------------------
// Packaging — build a real .morrow zip from the fixture cdylib
// ---------------------------------------------------------------------------

fn find_testmod_so() -> std::path::PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let p = crate_dir.join(format!("../target/{profile}/libtestmod.so"));
        if p.exists() {
            return p;
        }
    }
    panic!(
        "libtestmod.so not found — build the workspace first (`cargo build --release`)"
    );
}

/// Build `testmod.morrow` in `dir`. The platform dir is hardcoded to
/// linux-x86_64 (this test runs on Linux CI); keep in sync with
/// `Platform::dir_name()` in the runtime if it ever changes.
fn package_testmod(dir: &Path) -> std::path::PathBuf {
    let so = find_testmod_so();
    let pkg = dir.join("testmod.morrow");
    let file = std::fs::File::create(&pkg).unwrap();
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    zw.start_file("manifest.toml", opts).unwrap();
    zw.write_all(
        b"[package]\nname = \"testmod\"\nversion = \"0.1.0\"\n\n[morrow]\napi_version = 1\n\n[entry]\nsymbol = \"morrow_mod_init\"\n",
    )
    .unwrap();

    zw.start_file("linux-x86_64/libtestmod.so", opts).unwrap();
    zw.write_all(&std::fs::read(&so).unwrap()).unwrap();
    zw.finish().unwrap();
    pkg
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn log_contains(s: &str) -> bool {
    LOG.lock().unwrap().iter().any(|l| l.contains(s))
}

fn sent_contains(s: &str) -> bool {
    SENT.lock().unwrap().iter().any(|l| l.contains(s))
}

/// Batch payload in the Java `EventBuffer` wire format.
/// - tick: type=0, empty fields, 8-byte tick number
/// - join: type=1, field1 = player name
/// - chat: type=3, field1 = player, field2 = message
fn build_batch(events: &[BatchEvent]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&(events.len() as u32).to_le_bytes());
    for e in events {
        data.extend_from_slice(&e.kind.to_le_bytes());
        data.extend_from_slice(&(e.f1.len() as u16).to_le_bytes());
        data.extend_from_slice(&(e.f2.len() as u16).to_le_bytes());
        data.extend_from_slice(&e.f1);
        data.extend_from_slice(&e.f2);
    }
    data
}

struct BatchEvent {
    kind: u16,
    f1: Vec<u8>,
    f2: Vec<u8>,
}

fn tick_event(t: u64) -> BatchEvent {
    // tick number is read from the 8 bytes after the 6-byte header
    BatchEvent {
        kind: 0,
        f1: t.to_le_bytes().to_vec(),
        f2: Vec::new(),
    }
}

fn join_event(player: &str) -> BatchEvent {
    BatchEvent {
        kind: 1,
        f1: player.as_bytes().to_vec(),
        f2: Vec::new(),
    }
}

fn chat_event(player: &str, msg: &str) -> BatchEvent {
    BatchEvent {
        kind: 3,
        f1: player.as_bytes().to_vec(),
        f2: msg.as_bytes().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// The full cycle
// ---------------------------------------------------------------------------

#[test]
fn full_load_dispatch_cycle() {
    use morrow_runtime::*;

    let handle = morrow_init(ABI_VERSION);
    assert_ne!(handle, 0, "runtime init must return a valid handle");

    // Register the test host vtable (what Java does via upcall stubs)
    let vtable = HostVtable {
        get_player_count: Some(test_get_player_count),
        send_message: Some(test_send_message),
        get_player_list: Some(test_get_player_list),
        execute_command: Some(test_execute_command),
        get_world_time: Some(test_get_world_time),
        log_message: Some(test_log),
        get_world_snapshot: Some(test_get_world_snapshot),
    };
    morrow_register_host_api(handle, &vtable);

    // Package and load the fixture mod through the real loader
    let tmp = tempfile::tempdir().unwrap();
    let pkg = package_testmod(tmp.path());
    let path = pkg.to_str().unwrap();
    let result = morrow_load_mod(handle, path.as_ptr(), path.len() as u32);
    assert_eq!(result, 0, "mod load must succeed");

    // init ran: mod logged init-ok through the host vtable. The two
    // register_command calls also succeeded — any Err would have failed
    // init and made the load return non-zero above.
    assert!(log_contains("[testmod] init-ok"), "mod init must log: {:?}", *LOG.lock().unwrap());

    // Server start → server_start event
    morrow_dispatch_server_start(handle);
    assert!(log_contains("[testmod] server-start"));

    // Batch dispatch: tick 42 + player join alice + chat
    let batch = build_batch(&[tick_event(42), join_event("alice"), chat_event("bob", "hi")]);
    morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    assert!(log_contains("[testmod] tick-42"));
    assert!(log_contains("[testmod] join:alice"));
    assert!(log_contains("[testmod] chat:bob:hi"));

    // Unknown tick (not 42) must not fire the handler
    let batch2 = build_batch(&[tick_event(1)]);
    morrow_dispatch_batch(handle, batch2.as_ptr(), batch2.len() as u32);
    assert!(!log_contains("[testmod] tick-1"));

    // Command dispatch: registered command handled, args passed through
    let ping = b"testmod_ping";
    let handled =
        morrow_dispatch_command(handle, ping.as_ptr(), ping.len() as u32, b"hello".as_ptr(), 5);
    assert_eq!(handled, 1, "registered command must be handled");
    assert!(sent_contains("pong:hello"), "send_message must reach the host: {:?}", *SENT.lock().unwrap());

    // Command with host API round trip: player_count comes from the vtable
    let count = b"testmod_count";
    let handled =
        morrow_dispatch_command(handle, count.as_ptr(), count.len() as u32, b"".as_ptr(), 0);
    assert_eq!(handled, 1);
    assert!(sent_contains("players=7;args="));

    // Unknown command → 0
    let unknown = b"testmod_nope";
    let handled =
        morrow_dispatch_command(handle, unknown.as_ptr(), unknown.len() as u32, b"".as_ptr(), 0);
    assert_eq!(handled, 0, "unknown command must not be handled");

    // Duplicate registration is rejected: reloading the same package
    // re-runs init, whose register_command now conflicts → load fails.
    let second = morrow_load_mod(handle, path.as_ptr(), path.len() as u32);
    assert_ne!(second, 0, "duplicate command registration must reject the load");

    // Server stop → server_stop event
    morrow_dispatch_server_stop(handle);
    assert!(log_contains("[testmod] server-stop"));

    // Clean shutdown: drops the kernel, registry, and loaded libraries
    let shutdown = morrow_shutdown(handle);
    assert_eq!(shutdown, 0, "shutdown must succeed");
}
