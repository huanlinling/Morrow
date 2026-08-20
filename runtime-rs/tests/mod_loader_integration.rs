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

unsafe extern "C" fn test_get_world_snapshot(buf: *mut u8, cap: u32) -> u32 {
    write_snapshot(buf, cap, &["alice", "bob", "carol", "dave", "erin", "frank", "grace"], 12345)
}

// Serializes a WorldSnapshot in the host wire layout (u32 count, i64 time,
// u16-len-prefixed UTF-8 names) — same shape as ServerApiFabric.
fn write_snapshot(buf: *mut u8, cap: u32, names: &[&str], time: i64) -> u32 {
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&(names.len() as u32).to_le_bytes());
    out.extend_from_slice(&time.to_le_bytes());
    for name in names {
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    let n = out.len().min(cap as usize);
    unsafe { std::ptr::copy_nonoverlapping(out.as_ptr(), buf, n) };
    n as u32
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
///
/// Event type codes must stay in sync with the canonical table in
/// docs/02-abi-design.md §事件类型码 — the Java writer is pinned to the
/// same table by `bridge-java` EventBufferCodeTest. This test pins the
/// Rust parser side.
/// - tick:  type=0, 8-byte tick number after the header
/// - join/leave: type=1/2, field1 = player name
/// - chat:  type=3, field1 = player, field2 = message
/// - break/place: type=4/5, field1 = player, field2 = block
/// - death: type=6, field1 = player
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

fn leave_event(player: &str) -> BatchEvent {
    BatchEvent {
        kind: 2,
        f1: player.as_bytes().to_vec(),
        f2: Vec::new(),
    }
}

fn break_event(player: &str, block: &str) -> BatchEvent {
    BatchEvent {
        kind: 4,
        f1: player.as_bytes().to_vec(),
        f2: block.as_bytes().to_vec(),
    }
}

fn place_event(player: &str, block: &str) -> BatchEvent {
    BatchEvent {
        kind: 5,
        f1: player.as_bytes().to_vec(),
        f2: block.as_bytes().to_vec(),
    }
}

fn death_event(player: &str) -> BatchEvent {
    BatchEvent {
        kind: 6,
        f1: player.as_bytes().to_vec(),
        f2: Vec::new(),
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

    // Batch dispatch: all 7 event kinds in canonical code order — each
    // must route to exactly the right handler.
    let batch = build_batch(&[
        tick_event(42),
        join_event("alice"),
        leave_event("dave"),
        chat_event("bob", "hi"),
        break_event("carol", "stone"),
        place_event("carol", "dirt"),
        death_event("erin"),
    ]);
    morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    assert!(log_contains("[testmod] tick-42"));
    assert!(log_contains("[testmod] join:alice"));
    assert!(log_contains("[testmod] leave:dave"));
    assert!(log_contains("[testmod] chat:bob:hi"));
    assert!(log_contains("[testmod] break:carol:stone"));
    assert!(log_contains("[testmod] place:carol:dirt"));
    assert!(log_contains("[testmod] death:erin:"));

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

    // Command with host API round trip: player_count is now SNAPSHOT-backed
    // (not a direct vtable upcall) — this test never opens the consumer
    // gate, so the first read is the documented empty value. The positive
    // path is covered by snapshot_reads_open_the_gate_and_serve_cached_data.
    let count = b"testmod_count";
    let handled =
        morrow_dispatch_command(handle, count.as_ptr(), count.len() as u32, b"".as_ptr(), 0);
    assert_eq!(handled, 1);
    assert!(sent_contains("players=-1;args="));

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

// ---------------------------------------------------------------------------
// Snapshot consumer gate — reads are snapshot-backed, any-thread safe
// ---------------------------------------------------------------------------

static SNAP_CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

unsafe extern "C" fn test_snap_full(buf: *mut u8, cap: u32) -> u32 {
    SNAP_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    write_snapshot(buf, cap, &["alice", "bob"], 12345)
}

#[test]
fn snapshot_reads_open_the_gate_and_serve_cached_data() {
    use morrow_runtime::*;

    let handle = morrow_init(ABI_VERSION);
    assert_ne!(handle, 0);

    let vtable = HostVtable {
        get_player_count: Some(test_get_player_count),
        send_message: Some(test_send_message),
        get_player_list: Some(test_get_player_list),
        execute_command: Some(test_execute_command),
        get_world_time: Some(test_get_world_time),
        log_message: Some(test_log),
        get_world_snapshot: Some(test_snap_full),
    };
    morrow_register_host_api(handle, &vtable);

    // Gate CLOSED: no consumer has queried yet — the per-tick refresh
    // (and its upcall) must be skipped entirely.
    let batch = build_batch(&[tick_event(1)]);
    morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    assert_eq!(SNAP_CALLS.load(std::sync::atomic::Ordering::SeqCst), 0,
        "snapshot upcall must not run while the gate is closed");

    // First query: opens the gate, but the first refresh hasn't landed yet.
    assert_eq!(morrow_get_player_count(handle), -1);

    // Next tick: refresh runs (1 upcall) and the cache serves all reads.
    let batch = build_batch(&[tick_event(2)]);
    morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    assert_eq!(SNAP_CALLS.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(morrow_get_player_count(handle), 2);
    assert_eq!(morrow_get_world_time(handle), 12345);
    let mut buf = [0u8; 64];
    let n = morrow_get_player_list(handle, buf.as_mut_ptr(), buf.len() as u32);
    assert_eq!(&buf[..n as usize], b"alice,bob");

    // Reads never upcall: the count stays at 1 no matter how many queries.
    for _ in 0..100 {
        morrow_get_player_count(handle);
    }
    assert_eq!(SNAP_CALLS.load(std::sync::atomic::Ordering::SeqCst), 1);

    assert_eq!(morrow_shutdown(handle), 0);
}
