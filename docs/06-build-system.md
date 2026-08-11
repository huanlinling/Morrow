# 06 — 构建系统设计

## 开发环境（Docker）

Ferrum 使用 Docker 提供完全隔离、可复现的开发环境。不依赖宿主机上的任何 JDK 或 Rust 安装。

### 文件

```
ferrum/
├── Dockerfile            # 基于 eclipse-temurin:21-jdk + Rust stable
├── docker-compose.yml    # 挂载源码 + 持久化 Cargo 缓存
├── .dockerignore         # 精简构建上下文
└── Makefile              # 常用命令快捷入口
```

### 日常使用

```bash
# 第一次：构建 dev 镜像（下载 JDK + Rust，约 3 分钟）
docker compose build

# 进入开发环境
docker compose run --rm dev
# 你现在在一个安装了 JDK 21 + Rust 的容器里
# /ferrum 目录就是你的项目源码（实时同步）

# 直接在容器里跑命令
docker compose run --rm dev cargo build --release
docker compose run --rm dev cargo test

# 或者用 Makefile 快捷方式
make dev          # 进入容器
make build        # cargo build --release
make test         # cargo test
make clean        # 清理
```

### Dockerfile 解析

```dockerfile
FROM eclipse-temurin:21-jdk          # OpenJDK 21, TCK 认证构建
RUN apt-get install build-essential  # GCC, ld, libc — native 编译必需
RUN curl ... rustup | sh             # Rust stable toolchain
```

### docker-compose 关键设计

```yaml
volumes:
  - .:/ferrum                        # 源码即时同步，修改无需 rebuild
  - cargo-registry:/root/.cargo/registry  # crate 缓存持久化
  - cargo-git:/root/.cargo/git            # git 依赖缓存
  - cargo-target:/ferrum/runtime-rs/target # 编译缓存持久化
```

三个 Cargo 缓存 volume 是关键 — 否则每次 `docker compose run` 都是全新容器，`cargo build` 要从头下载编译所有依赖，浪费大量时间。

### 为什么 Docker 适合 Ferrum

| 问题 | Docker 怎么解决 |
|------|----------------|
| glibc 版本不一致 | Docker image 固定 Ubuntu glibc 版本 |
| "我机器上能跑" | 所有人的环境完全一致（image hash） |
| CI 行为不同 | CI 用同一个 Dockerfile |
| 新人装环境痛苦 | 装 Docker → `docker compose run dev` 就绪 |
| 污染宿主机 | 所有工具链都在容器里 |

## 构建工具链总览

```
┌─────────────────────────────────────┐
│            Ferrum Build              │
│                                      │
│  Rust       Java          Package    │
│  Cargo      Gradle+Loom   Ferrum CLI │
│    │           │              │       │
│    ▼           ▼              ▼       │
│  .so/.dll   .jar           .ferrum   │
│  (cdylib)   (Fabric mod)   (ZIP)     │
└─────────────────────────────────────┘
```

## Rust 侧：Cargo Workspace

### workspace Cargo.toml

```toml
# ferrum/Cargo.toml
[workspace]
resolver = "2"
members = [
    "runtime-rs",
    "sdk-rs",
    "sdk-rs/ferrum-macros",
    "examples/hello-ferrum",
    "ferrum-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/ferrum-mc/ferrum"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
libloading = "0.8"
zip = "2"
toml = "0.8"
```

### runtime-rs/Cargo.toml

```toml
[package]
name = "ferrum-runtime"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib"]    # 编译为 .so/.dll
name = "ferrum_runtime"    # 输出: libferrum_runtime.so

[dependencies]
# 最小依赖原则 — runtime 要尽可能轻量
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

# 不使用 async runtime
# 不使用 tokio
# 不使用大型框架

[profile.release]
opt-level = 3              # 最大优化
lto = "fat"                # 全 LTO
codegen-units = 1          # 更好的内联
panic = "abort"            # 不依赖 panic unwind
strip = "symbols"          # 减小 .so 体积
```

### sdk-rs/Cargo.toml

```toml
[package]
name = "ferrum"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["rlib"]      # 静态链接到 mod 中
name = "ferrum"

[dependencies]
ferrum-macros = { path = "ferrum-macros" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

### ferrum-macros/Cargo.toml

```toml
[package]
name = "ferrum-macros"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"
```

## Java 侧：Gradle + Loom

### bridge-java/settings.gradle

```groovy
pluginManagement {
    repositories {
        maven {
            name = 'Fabric'
            url = 'https://maven.fabricmc.net/'
        }
        gradlePluginPortal()
    }
}
```

### bridge-java/fabric-host/build.gradle

```groovy
plugins {
    id 'fabric-loom' version '1.7-SNAPSHOT'
    id 'maven-publish'
}

version = project.mod_version
group = project.maven_group

base {
    archivesName = project.archives_base_name
}

dependencies {
    // Fabric
    minecraft "com.mojang:minecraft:${project.minecraft_version}"
    mappings "net.fabricmc:yarn:${project.yarn_mappings}:v2"
    modImplementation "net.fabricmc:fabric-loader:${project.loader_version}"
    modImplementation "net.fabricmc.fabric-api:fabric-api:${project.fabric_version}"

    // No Panama dependencies needed — it's in JDK 21 stdlib
}

processResources {
    // 将 Rust 构建的 .so 文件复制到 JAR 中
    from("${rootProject.projectDir}/../runtime-rs/target/release") {
        include "*.so", "*.dll"
        into "natives"
    }
}

// 自定义任务：构建 Rust runtime
tasks.register('buildRustRuntime', Exec) {
    workingDir "${rootProject.projectDir}/../runtime-rs"
    commandLine 'cargo', 'build', '--release'
}

// 让 Java 构建依赖 Rust 构建
tasks.named('processResources').configure {
    dependsOn 'buildRustRuntime'
}
```

### gradle.properties

```properties
minecraft_version=1.20.1
yarn_mappings=1.20.1+build.10
loader_version=0.16.5
fabric_version=0.92.0+1.20.1

mod_version=0.1.0
maven_group=com.ferrum
archives_base_name=ferrum-host
```

## 构建流程

### 开发构建

```bash
# 1. 构建 Rust runtime
cd runtime-rs
cargo build --release
# → target/release/libferrum_runtime.so

# 2. 构建 Java bridge（自动复制 .so）
cd bridge-java/fabric-host
./gradlew build
# → build/libs/ferrum-host-0.1.0.jar

# 3. 构建示例 mod
cd examples/hello-ferrum
cargo build --release
# → target/release/libhello_ferrum.so

# 4. 打包为 .ferrum
cd ../..
cargo run --bin ferrum-cli -- package ./examples/hello-ferrum
# → hello-ferrum.ferrum
```

### CI 构建（全自动）

```yaml
# .github/workflows/build.yml
name: Build
on: [push, pull_request]

jobs:
  build-runtime:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cd runtime-rs && cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: runtime-${{ matrix.os }}
          path: runtime-rs/target/release/*.{so,dll}

  build-java:
    runs-on: ubuntu-latest
    needs: build-runtime
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with:
          java-version: 21
          distribution: temurin
      - run: cd bridge-java/fabric-host && ./gradlew build
```

## 开发工作流

### 快速迭代

```bash
# 只改 Rust runtime
cd runtime-rs && cargo build --release
cp target/release/libferrum_runtime.so \
   ~/.minecraft/mods/ferrum/native/linux-x86_64/
# 重启 Minecraft

# 只改 SDK
cd sdk-rs && cargo build
cd examples/hello-ferrum && cargo build --release
ferrum-cli package . --output ~/.minecraft/mods/
# 重启 Minecraft

# 只改 Java bridge
cd bridge-java/fabric-host && ./gradlew build
cp build/libs/ferrum-host-0.1.0.jar ~/.minecraft/mods/
# 重启 Minecraft
```

### 调试

```bash
# Rust 调试构建（带符号、无优化）
cd runtime-rs && cargo build  # debug mode
# Java attach native debugger
java -agentlib:native-debugger ... # 需要专门的 native 调试工具

# 更实际的调试方式：
# - Rust 侧用 tracing 日志，输出到 stderr
# - Java 侧用 log4j，输出到 Minecraft log
# - 跨 FFI 的问题用 error channel 传递
```

## 版本管理

```
Ferrum 版本号:
  runtime-rs:   独立版本（遵循 semver）
  sdk-rs:       与 runtime 主版本同步
  bridge-java:  独立版本（Java 侧有自己的发布节奏）
  ABI version:  独立版本（仅在不兼容变更时递增）
```

建议的发布节奏：

```
runtime-rs 0.1.0  ←  M1 完成
runtime-rs 0.2.0  ←  M3 完成
runtime-rs 0.3.0  ←  M5 完成
runtime-rs 1.0.0  ←  v1 完成
```
