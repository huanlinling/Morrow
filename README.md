# Morrow

> Native Minecraft Mod Loader — write mods in Rust, run at native speed.

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![JDK](https://img.shields.io/badge/JDK-21%2B-red.svg)](https://adoptium.net)

## What is Morrow?

Morrow is an **independent native mod loader** for Minecraft. Write mods in Rust, compiled to native code, loaded via Panama FFI. No Fabric API required.

- 🦀 **Rust SDK** — `#[morrow::mod_main]`, Context API, log macros
- ⚡ **1 FFM call/tick** — batch event dispatch, cost independent of mod count
- 🔒 **Panic isolation** — mod crash never takes down the server
- 📦 **Cross-platform** — `.morrow` packages per OS/arch
- 🔌 **Independent** — Java Agent + Mixin, no mod framework dependency

## Quick Start

### Prerequisites

| Component | Version |
|-----------|---------|
| JDK | OpenJDK 21+ |
| Rust | stable (1.80+) |
| Minecraft | 1.20.1 |

### Write a mod

```rust
use morrow::prelude::*;

#[morrow::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
    morrow::info!("Hello from Rust!");
    Ok(())
}
```

### Run

```bash
# Build the runtime
make build

# Package example mod
make package-hello

# Run Minecraft (dev mode)
cd bridge-java && ./gradlew runServer

# Production: java -javaagent:morrow.jar -jar server.jar
```

## Architecture

```
java -javaagent:morrow.jar -jar server.jar
  │
  ├── Morrow Agent (premain)
  │     └── Mixin → MinecraftServer (loadWorld / tick / shutdown)
  │           └── EventBuffer → 1 FFM/tick batch dispatch
  │
  └── Panama FFI (~10ns/downcall)
        └── Rust Runtime (libmorrow_runtime.so)
             ├── parse batch → dispatch to mods
             ├── panic quarantine
             ├── Host API (6 upcalls)
             └── mod A, B, C...
```

## API Surface

### RuntimeApi (mod → runtime)

| Function | Description |
|----------|-------------|
| `get_player_count` | Online players |
| `get_player_list` | Player names |
| `send_message` | Broadcast to chat |
| `execute_command` | Run server command |
| `get_world_time` | World time in ticks |
| `register_command` | Register `/` command |
| `get_config` | Read config.toml |
| `request_capability` | Check feature |
| `log` | Structured logging |

### Optional mod exports

| Export | Called when |
|--------|-------------|
| `morrow_mod_tick(tick)` | Every tick (20 TPS) |
| `morrow_mod_server_start()` | Server started |
| `morrow_mod_server_stop()` | Server stopping |
| `morrow_mod_player_join(name)` | Player joins |
| `morrow_mod_player_leave(name)` | Player leaves |
| `morrow_mod_chat_message(player, msg)` | Chat sent |
| `morrow_mod_block_break(player, block)` | Block broken |
| `morrow_mod_block_place(player, block)` | Block placed |
| `morrow_mod_player_death(player, msg)` | Player dies |

## Project Structure

```
morrow/
├── runtime-rs/          # Rust Runtime Core (cdylib)
├── sdk-rs/              # Rust SDK for mod developers
│   └── morrow-macros/   # #[morrow::mod_main] proc macro
├── bridge-java/         # Java Agent + Mixin + Panama bridge
├── examples/
│   ├── hello-morrow/    # Full API demo mod
│   └── chat-bot/        # Chat bot mod
├── scripts/             # Build & packaging
└── docs/                # Design documents
```

## Status

| Milestone | Status |
|-----------|--------|
| M0-M7: Core Platform | ✅ |
| v0.8: Windows CI, docs, examples | ✅ |
| v0.9: Logging system | ✅ |
| v0.10: Mod dependency resolution | ✅ |
| v0.11: Independent loader (Mixin) | ✅ |
| v0.12: Java Agent + batch dispatch | ✅ |
| v0.13: Zero-copy event parsing | ✅ |
| v0.14: PlayerSnapshot | ⬜ Next |

## Development

```bash
make build          # Build Rust runtime + mods
make test           # Run Rust unit tests
make test-bridge    # Run Panama bridge tests
make package-hello  # Package hello-morrow.morrow
```

## License

Dual-licensed under MIT and Apache 2.0.
