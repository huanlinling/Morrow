# 06 — 构建系统（v0.16 现状）

## 工具链

| 组件 | 版本 | 用途 |
|------|------|------|
| JDK | Temurin 21（`--enable-preview`，Panama 在 21 仍 preview） | Java host + 桥接测试 |
| Rust | stable 1.80+ | runtime / SDK / 示例 mod |
| Gradle | wrapper 8.10（自动下载）+ fabric-loom 1.7.4 | dev 模式 runServer + Agent JAR |
| Python 3 | — | `scripts/package-mod.sh` 打包 .mor |
| make/bash/gcc | — | Makefile 编排 + Rust 链接 |

## 构建产物与入口

```
Makefile
├── make build          cargo build --release（runtime + 3 个示例 mod）
├── make test           cargo test（单元 + 集成）
├── make test-bridge    bash bridge-java/build.sh（Panama 桥接测试）
├── make package-hello  scripts/package-mod.sh × 3 → *.mor 包
└── make clean

bridge-java/build.sh（无 Gradle 依赖，javac 直编）:
  1. cargo build --release
  2. javac（PanamaBridge + EventBuffer + M0/M1/CodeTest/Benchmark）
  3. M0 add 回归 → 4. M1 生命周期 → 5. 事件码 parity → 6. 基准

bridge-java/gradle（fabric-loom，两个用途）:
  ├─ ./gradlew runServer    dev 冒烟：自动下载 MC 1.20.1 + Yarn + Fabric
  │                        Loader dev 运行时（Loader 仅当类加载器）
  └─ ./gradlew build        Agent JAR（Premain-Class: com.morrow.agent.MorrowAgent，
                            processResources 内嵌 .so → natives/<platform>/）
```

生产入口：`java -javaagent:morrow.jar -jar server.jar`（不依赖 Fabric）。

## Cargo workspace

```
members = runtime-rs, runtime-rs/tests/fixtures/testmod,
          sdk-rs, sdk-rs/morrow-macros,
          examples/{hello-morrow, chat-bot, motd}
```

runtime-rs：`crate-type = ["cdylib", "rlib"]`（rlib 供集成测试链接，
cdylib 进镜像）；依赖刻意最小——zip/toml/serde/libloading/log/tempfile，
无 async runtime、无 tokio。

## Docker（3 阶段，Dockerfile）

```
builder (eclipse-temurin:21-jdk-noble + Rust + Gradle)
  └─ cargo build+test → gradlew build → package mods → make test-bridge
      ├── runtime (21-jre-noble)：生产镜像 = Agent JAR + .so + mods/
      │     入口 entrypoint.sh，需挂载 server.jar（Mojang 分发条款）
      └── dev（默认目标）：基于 builder，loom runServer 即开即测
            入口 dev-entrypoint.sh，自动下载 MC，EULA=true 可跑
```

设计要点：builder 阶段跑完所有测试再进 runtime 镜像，生产镜像不含
工具链；mods/ 目录挂载覆盖内置示例。

## CI（.github/workflows/ci.yml）

Ubuntu 单 job：
1. `cargo build --release` + `cargo test --release`
2. `scripts/package-mod.sh` × 3 打包示例
3. `./gradlew --no-daemon build`（bridge-java，内嵌 .so）
4. `make test-bridge`（Panama 桥接测试）
5. `nm -D` 校验导出符号（runtime 5 个核心符号 + hello-morrow 10 个
   `morrow_mod_*` + testmod 8 个）——符号契约回归防线

## 版本管理

```
runtime-rs / sdk-rs: 独立 semver
bridge-java:         独立版本（morrow-host）
ABI version:         独立（0x0001_0000，仅不兼容变更递增主版本）
```
