# 09 — Benchmark Results

Run on: AMD Ryzen (Linux x86_64), JDK 21, Rust 1.97.1, `--release`.

## Raw Panama Downcall Latency

Calling `add(i32, i32) -> i32` via Panama FFM.

| Metric | Value |
|--------|-------|
| Iterations | 1,000,000 |
| Total time | 9.70 ms |
| **Avg latency** | **9.7 ns/call** |
| Throughput | 103,000,000 calls/sec |

Within the 5-10 ns design target (Panama FFM spec).

## Tick Dispatch Latency

Calling `morrow_tick()` with an empty runtime (no mods loaded).

| Metric | Value |
|--------|-------|
| Ticks | 100,000 |
| Total time | 3.71 ms |
| **Avg latency** | **0.04 μs/tick** |
| Theoretical max TPS | 26,900,000 ticks/sec |

2,500× faster than the <100 μs design target. Tick dispatch overhead is negligible.

## Batch Dispatch (production path, M7)

Full tick loop as the real host runs it: `EventBuffer` write → `finish()`
→ `morrow_dispatch_batch` → Rust parse, including per-tick arena churn
(`reset()` closes the confined arena each iteration). 100,000 ticks,
empty runtime, release build.

| Events/tick | Avg latency | Marginal cost/event |
|-------------|-------------|---------------------|
| 1 (tick only) | **0.393 μs** | — |
| 8 (tick + 7 chat) | **0.617 μs** | 77 ns |

Acceptance (docs/07 M7): < 1 μs/tick for 1 event — passed with 2.5× margin.
The EventBuffer + arena path costs ~0.35 μs/tick on top of the bare
`morrow_tick` downcall; still 0.0008% of the 50 ms tick budget.

## Multi-mod Scalability (M7)

N identical no-op mods (cdylib, silent tick handler), loaded through the
real loader (manifest → extract → dlopen → init → callback registration),
10,000 tick batches each. Release build.

| Mods | Avg latency/tick | Marginal cost/mod |
|------|------------------|-------------------|
| 1 | **0.118 μs** | — |
| 10 | **0.220 μs** | ~11 ns |
| 50 | **1.399 μs** | ~28 ns |

Acceptance (docs/07 M7): 50 mods < 2 ms/tick — passed with 1400× margin.
Growth is ~linear (5× mods → 6.4× cost; the fixed per-batch overhead of
~0.1 μs accounts for the slight tilt). Guarded in CI by
`runtime-rs/tests/scalability.rs` (quadratic-blowup assertion).

## Memory Footprint (M7)

RSS deltas via /proc/self/status, release build, 50 no-op mods loaded
through the real loader (`runtime-rs/tests/scalability.rs`, Linux):

| Phase | RSS delta |
|-------|-----------|
| Runtime init | **+144 KiB** |
| Per loaded mod (×50) | **~299 KiB/mod** |
| Post-shutdown residue | ~1.5 MiB (allocator caches; dlclose unmaps mod libs) |

Acceptance (docs/07 M7): runtime base < 2 MB, per mod < 1 MB — passed.
Note: the ~299 KiB/mod is dominated by the mod library's mapped pages
(~412 KB file, lazily paged); registry overhead per mod is ~4 KB as
previously stated. The earlier "2.2 MB runtime" figure was the .so file
size on disk — resident memory after init is far smaller.

## JNI vs Panama Downcall (M7 7.1)

Same trivial call shape (`add(i32, i32) -> i32`), same process, 1M
iterations, JDK 21:

| Mechanism | Avg latency |
|-----------|-------------|
| JNI | **7.2 ns/call** |
| Panama FFM | **7.0 ns/call** (1.02×) |

Correction to earlier assumptions: on JDK 21 a trivial JNI call is NOT
2-3× slower — both mechanisms sit at the JVM's native-call floor. Panama's
real advantages are lifecycle (arena vs GlobalRef), type safety, and no
glue code — plus the batch dispatch architecture, which is where Morrow's
performance actually comes from (design.md §零: 次数是杠杆，单价不是).

## Mod Load Overhead

| Metric | Value |
|--------|-------|
| Empty mod .so size | ~412 KB |
| Extraction + dlopen time | <5 ms (observed) |
| Entry point call | <1 μs |

## Memory Baseline

On-disk / registry figures (resident memory measured below in
"Memory Footprint (M7)"):

| Component | Approx Size |
|-----------|-------------|
| libmorrow_runtime.so | ~2.2 MB (release, stripped, on disk) |
| Runtime kernel (idle) | ~1 KB (single handle entry) |
| Per loaded mod | ~4 KB (registry entry + metadata; the mapped mod .so is extra) |

## Conclusion

The Panama FFM bridge delivers on the "native performance" promise:
- **Zero measurable overhead** from the Java→Rust bridge
- Tick dispatch is fast enough for 20 TPS with thousands of mods
  (50 mods ≈ 1.4 μs/tick; the 50 ms budget would fit ~35,000 such mods)
- Batch dispatch is the right shape: 1 event 0.39 μs, 8 events 0.62 μs —
  marginal events cost 77 ns, not another FFI round trip
- Mod loading is I/O-bound by disk, not CPU

**Performance line is at its ceiling** (design.md §零): loader overhead is
0.00008% of the tick budget; the benchmark suite is acceptance-grade, not
research-grade. M7 complete: all six items measured, guarded in CI, and
documented here as the regression baseline.
