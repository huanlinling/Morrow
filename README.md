# Ferrum

> Minecraft Native Runtime Platform — Write Minecraft mods in Rust, coexist with Java mods, zero compromise on performance.

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![JDK](https://img.shields.io/badge/JDK-21%2B-red.svg)](https://adoptium.net)

## What is Ferrum?

Ferrum is a **native runtime platform** that lets you write Minecraft mods in Rust, compiled to native code, running alongside existing Java mods on Fabric.

- 🦀 **Rust SDK** — Write mods with proc macros and safe abstractions
- 🧵 **Java coexistence** — Runs as a Fabric mod; interoperates with any Java mod
- ⚡ **Native performance** — Project Panama FFI, minimal overhead
- 🔒 **Panic isolation** — Rust mod crashes never take down the server
- 📦 **Cross-platform** — `.ferrum` packages contain native artifacts per OS/arch

## Quick Start

### Prerequisites

| Component | Version |
|-----------|---------|
| JDK | OpenJDK 21+ (e.g. Eclipse Temurin) |
| Rust | stable (1.80+) |
| Minecraft | 1.20.1 |
| Fabric Loader | 0.16+ |

### Write a mod

```rust
use ferrum::prelude::*;

#[ferrum::mod_main]
fn init(_ctx: &mut Context) -> Result<(), FerrumError> {
    ferrum::info!("Hello from Rust!");
    Ok(())
}
```

### Build & Run

```bash
# Build everything
make build

# Package the example mod
make package-hello

# Run Minecraft with Ferrum
cd bridge-java && ./gradlew runServer
```

## Architecture

```
Minecraft (Java) → Fabric Host Adapter → Panama FFI → Ferrum Runtime (Rust)
                                                          ├── Mod A (.ferrum)
                                                          ├── Mod B (.ferrum)
                                                          └── Mod C (.ferrum)
```

## Project Structure

```
ferrum/
├── runtime-rs/          # Ferrum Runtime Core (Rust cdylib)
├── sdk-rs/              # Ferrum SDK for mod developers
│   └── ferrum-macros/   # Proc macros (#[ferrum::mod_main])
├── bridge-java/         # Fabric Host Adapter (Java + Panama + Gradle)
├── examples/
│   └── hello-ferrum/    # The simplest possible mod
├── scripts/             # Build & packaging utilities
└── docs/                # Design documents
```

## Status

| Milestone | Status |
|-----------|--------|
| M0: Environment + First Panama Call | ✅ Done |
| M1: Minimal Runtime (init/shutdown) | ✅ Done |
| M2: Fabric Integration | ✅ Done |
| M3: Rust Mod Loading | ✅ Done |
| M4: Event Dispatch | ✅ Done |
| M5: SDK Macros | ✅ Done |
| M6: Linux Verification | ✅ Done |
| M7: Benchmark Suite | ✅ Done |
| M8: Windows Support | 🔨 In Progress |

## Development

### Linux & macOS

```bash
make build          # Build Rust runtime
make test           # Run Rust unit tests
make test-bridge    # Run Panama bridge tests (M0 + M1)
make package-hello  # Package the example mod
cd bridge-java && ./gradlew runServer  # Run test server
```

### Windows

```powershell
cargo build --release
cargo test
cd bridge-java; ./gradlew runServer

# Package example mod
bash scripts/package-mod.sh examples/hello-ferrum  # Git Bash / WSL
# or manually create a ZIP with manifest + DLL
```

## License

Dual-licensed under MIT and Apache 2.0.
