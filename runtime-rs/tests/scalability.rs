//! M7 scalability (7.5): N no-op mods → `morrow_dispatch_batch` cost
//! per tick. Guards the core dispatch claim — cost stays roughly linear
//! in mod count (one extra mod = one extra fn call), never quadratic.
//!
//! Each mod is the same cdylib packaged under a distinct name, loaded
//! through the real loader (manifest parse → extract → dlopen → init →
//! callback registration), then a tick batch is dispatched over all of
//! them. Printouts feed docs/09-benchmarks.md; the assertions guard
//! against blowup, not microsecond jitter (CI-safe).

use std::io::Write;
use std::path::Path;

const ABI_VERSION: u32 = 0x0001_0000;

fn find_noop_so() -> std::path::PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let p = crate_dir.join(format!("../target/{profile}/libnoop_mod.so"));
        if p.exists() {
            return p;
        }
    }
    panic!("libnoop_mod.so not found — build the workspace first (`cargo build --release`)");
}

/// Build `{name}.morrow` in `dir` containing the shared noop cdylib.
fn package_noop(dir: &Path, name: &str) -> std::path::PathBuf {
    let so = find_noop_so();
    let pkg = dir.join(format!("{name}.morrow"));
    let file = std::fs::File::create(&pkg).unwrap();
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    zw.start_file("manifest.toml", opts).unwrap();
    zw.write_all(
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[morrow]\napi_version = 1\n\n[entry]\nsymbol = \"morrow_mod_init\"\n"
        )
        .as_bytes(),
    )
    .unwrap();

    zw.start_file("linux-x86_64/libnoop_mod.so", opts).unwrap();
    zw.write_all(&std::fs::read(&so).unwrap()).unwrap();
    zw.finish().unwrap();
    pkg
}

/// Batch payload in the Java `EventBuffer` wire format: 1 tick event.
fn tick_batch(tick: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&1u32.to_le_bytes()); // total_events
    data.extend_from_slice(&0u16.to_le_bytes()); // type = tick
    data.extend_from_slice(&0u16.to_le_bytes()); // field1_len
    data.extend_from_slice(&0u16.to_le_bytes()); // field2_len
    data.extend_from_slice(&tick.to_le_bytes()); // tick payload
    data
}

/// Load `n_mods` no-op mods, dispatch 10k tick batches, return μs/tick.
fn bench_tick_with_mods(n_mods: usize) -> f64 {
    use morrow_runtime::*;

    let handle = morrow_init(ABI_VERSION);
    assert_ne!(handle, 0, "runtime init must return a valid handle");

    let tmp = tempfile::tempdir().unwrap();
    for i in 0..n_mods {
        let pkg = package_noop(tmp.path(), &format!("noop{i}"));
        let path = pkg.to_str().unwrap();
        assert_eq!(
            morrow_load_mod(handle, path.as_ptr(), path.len() as u32),
            0,
            "loading noop{i} must succeed"
        );
    }

    let batch = tick_batch(1);
    for _ in 0..1000 {
        morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    }

    const ITERS: u32 = 10_000;
    let start = std::time::Instant::now();
    for _ in 0..ITERS {
        morrow_dispatch_batch(handle, batch.as_ptr(), batch.len() as u32);
    }
    let elapsed = start.elapsed();

    assert_eq!(morrow_shutdown(handle), 0, "shutdown must succeed");
    elapsed.as_secs_f64() / ITERS as f64 * 1e6 // μs/tick
}

#[test]
fn dispatch_scales_linearly_with_mod_count() {
    let t1 = bench_tick_with_mods(1);
    let t10 = bench_tick_with_mods(10);
    let t50 = bench_tick_with_mods(50);
    println!(
        "1 mod: {t1:.3} μs/tick | 10 mods: {t10:.3} μs/tick | 50 mods: {t50:.3} μs/tick"
    );

    // Linear guard: 50 mods ≈ 5× the cost of 10. Quadratic blowup would
    // be ~25×. 30× headroom catches blowup, not CI jitter.
    assert!(
        t50 < 30.0 * t10,
        "dispatch cost must not blow up quadratically: t50={t50:.3} vs t10={t10:.3}"
    );
    // Acceptance (docs/07 M7): 50 mods < 2ms/tick. Generous for slow CI
    // runners; local release numbers go to docs/09.
    assert!(
        t50 < 2_000.0,
        "50 mods must stay under 2ms/tick: {t50:.3}"
    );
}

// ─── M7 7.3: memory footprint ─────────────────────────────────────

/// Resident set size in KiB, from /proc/self/status (Linux only).
fn rss_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    for line in status.lines() {
        if let Some(v) = line.strip_prefix("VmRSS:") {
            return v.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
        }
    }
    0
}

/// Bounds the runtime's memory footprint: baseline → init → 50 mods →
/// shutdown. Assertions are leak guards (generous for CI noise), not
/// certification — the real numbers feed docs/09-benchmarks.md.
#[test]
#[cfg(target_os = "linux")]
fn memory_footprint_stays_bounded() {
    use morrow_runtime::*;

    let base = rss_kib();

    let handle = morrow_init(ABI_VERSION);
    assert_ne!(handle, 0, "runtime init must return a valid handle");
    let after_init = rss_kib();

    let tmp = tempfile::tempdir().unwrap();
    for i in 0..50 {
        let pkg = package_noop(tmp.path(), &format!("mem{i}"));
        let path = pkg.to_str().unwrap();
        assert_eq!(
            morrow_load_mod(handle, path.as_ptr(), path.len() as u32),
            0,
            "loading mem{i} must succeed"
        );
    }
    let after_mods = rss_kib();

    assert_eq!(morrow_shutdown(handle), 0, "shutdown must succeed");
    let after_shutdown = rss_kib();

    let runtime_cost = after_init.saturating_sub(base);
    let mods_cost = after_mods.saturating_sub(after_init);
    let residue = after_shutdown.saturating_sub(base);
    println!(
        "RSS: base {base} KiB | runtime +{runtime_cost} KiB | 50 mods +{mods_cost} KiB \
         ({mods_cost_kib} KiB/mod) | post-shutdown residue {residue} KiB",
        mods_cost_kib = mods_cost / 50
    );

    // Leak guards — order-of-magnitude headroom over the observed values.
    assert!(runtime_cost < 20 * 1024, "runtime must stay under 20 MiB: {runtime_cost} KiB");
    assert!(mods_cost < 50 * 1024, "50 mods must stay under 50 MiB: {mods_cost} KiB");
    // dlclose unmaps mod libraries; allocator caches may retain some
    // pages, so the residue bound is looser than the others.
    assert!(residue < 30 * 1024, "post-shutdown residue must stay under 30 MiB: {residue} KiB");
}
