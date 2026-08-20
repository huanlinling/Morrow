# Changelog

## [Unreleased]

### Runtime (Rust)

- Player/world query APIs are snapshot-backed: `player_count`, `player_list`,
  `world_time` serve a per-tick `WorldSnapshot` cache instead of calling
  into the game directly. First query opens a consumer gate (refresh lands
  ≤ 1 tick later, first read returns the empty value); the 64 KiB snapshot
  buffer is kernel-owned and reused across ticks. Reads are now safe from
  ANY thread — this supersedes the 1.0.0 "main-thread reads" contract.

### Host (Java agent)

- Agent jar renamed Forge-style: `morrow-host-1.20.1-1.0.0-agent.jar`
  (the obfuscated mixin names are pinned to MC 1.20.1, so the artifact
  name carries the target version)
- Place events report the block actually placed (click position + face
  offset), not the block clicked at
- Hermetic end-to-end harness: boots a real vanilla 1.20.1 server, drives
  join/chat/death/leave via a protocol-level fake client and break/place
  via a self-test mixin (`-Dmorrow.selftest.place=true`) — all nine event
  kinds verified on a real server; wired into CI as the `agent-e2e` job
- Vanilla mixins compile at release 17, matching the config's JAVA_17
  compatibility level — the one-time startup "class version" WARN is gone
  (mixin 0.8.5's CompatibilityLevel tops out at JAVA_18; ServerApiVanilla
  keeps FFM preview at release 21 in its own compile task)

## [1.0.0] — 2026-08-19

First stable release. Rust mods on a vanilla Minecraft 1.20.1 server, no
Fabric required.

### Runtime (Rust)

- Batch event dispatch: one Panama FFM call per tick regardless of mod or
  event count (0.32 μs/tick measured, docs/09)
- Per-tick dispatch tables behind a single `Arc` snapshot; world-snapshot
  refresh is consumer-gated (zero cost until a query API exists)
- Panic isolation with per-mod quarantine; a panicking mod never takes
  down the server
- `send_message` / `execute_command` are safe from mod-spawned threads —
  writes are marshaled onto the game main thread at the next tick
- Mod loading from `.morrow` ZIP packages (dlopen), dependency ordering
  with retry, per-mod `config.toml`, capability negotiation
- Command registration (`/ping`-style) with re-entrant dispatch

### SDK (sdk-rs + morrow-macros)

- `#[morrow::mod_main]` entry point, `#[morrow::event(...)]` for all nine
  event kinds (tick, start, stop, join, leave, chat, break, place, death)
- Global API (`send_message`, `player_count`, `config`, log macros, ...)
  usable from any thread for writes, main-thread handlers for reads
- Zero-copy `read_str` for event payloads

### Host (Java agent)

- Standalone agent mode: `java -javaagent:morrow-host-1.0.0-agent.jar
  -jar server.jar` — no Fabric Loader on the classpath
- Self-hosted vanilla Mixin service; obfuscated-name mixin + adapter for
  the production jar (1.20.1, javap-verified)
- `HostLink` + `ChildFirstLoader` classloading: agent classes visible to
  the game loader, Mojang signer check avoided
- Dev mode unchanged (Fabric Loom `runServer`, yarn names)
- Game-free host core behind a `ServerApi` adapter per mode

### Verification

- 18 runtime tests + integration + scalability guards (CI)
- Full bridge suite in CI: M0/M1 regression, event-code parity,
  benchmarks, JNI-vs-Panama comparison, agent premain smoke
- End-to-end on a real 1.20.1 server: mixin applied → 3 mods loaded →
  tick events → clean shutdown

[1.0.0]: https://github.com/huanlinling/Morrow/releases/tag/v1.0.0
