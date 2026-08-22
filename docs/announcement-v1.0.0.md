# Morrow v1.0.0 — Write Minecraft mods in Rust, at native speed

**Morrow** is an independent native mod loader for Minecraft. Mods are
written in Rust, compiled to native code, and loaded into a vanilla
server through a Java agent — no Fabric, no Forge, no mod framework on
the classpath.

```bash
java --enable-preview --enable-native-access=ALL-UNNAMED \
     --add-opens java.base/java.net=ALL-UNNAMED \
     -javaagent:morrow-host-1.20.1-1.0.1-agent.jar \
     -jar server.jar nogui
```

## Why

Java modding APIs make you pay the JVM's tax for every event, and the
loader's version chain for every Minecraft update. Morrow asks a
different question: what if a mod were just a native library with
function pointers?

- **1 FFI call per tick.** Events accumulate off-heap and cross the
  Java↔Rust boundary once per tick, batched — the cost is independent
  of how many mods or events you have. Measured: **0.32 μs/tick**,
  0.0006% of the 50 ms tick budget.
- **A crashing mod can't take down the server.** Every callback is
  panic-isolated; a panicking mod is quarantined and skipped while
  everyone else keeps playing.
- **No mod framework dependency.** The host is a Java agent with one
  Mixin injection point. It runs on a completely vanilla 1.20.1 server
  jar — obfuscated names and all.

## What a mod looks like

```rust
use morrow::prelude::*;

#[morrow::mod_main]
fn init(_ctx: &mut Context, _api: *const RuntimeApi) -> Result<(), MorrowError> {
    morrow::info!("Hello from native code!");
    Ok(())
}

#[morrow::event(chat_message)]
fn on_chat(player: &str, msg: &str) {
    if msg == "hi" {
        morrow::send_message(&format!("Hello, {player}!"));
    }
}
```

`morrow build` gives you a `.mor` package. Drop it in `mods/`. Done.

## Under the hood

- **Panama FFM (JDK 21)** for the bridge — measured at parity with JNI
  (7.0 vs 7.2 ns/call), with safe arena-based memory management instead
  of GlobalRef bookkeeping.
- **Symbol-discovery ABI** — a mod is a cdylib that exports
  `morrow_mod_tick`, `morrow_mod_chat_message`, ...; no registration
  protocol, no version negotiation beyond one ABI check.
- **Thread-safe by design** — writes from mod-spawned threads are
  marshaled onto the game main thread automatically; the world snapshot
  is refreshed once per tick instead of per query.
- **Benchmarks are CI-enforced** — the numbers above aren't marketing,
  they're a regression baseline that fails the build if it moves.

## Status

v1.0.0 targets Minecraft 1.20.1 (JDK 21+). Three example mods ship in
the repo. Everything is MIT/Apache-2.0.

- GitHub: https://github.com/MorrowMC/Morrow
- SDK: `cargo add morrow`
- Docs: architecture, ABI, and benchmark methodology in `docs/`

Write your next mod in Rust. The server won't notice it's there —
that's the point.
