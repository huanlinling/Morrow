# 07 — 路线图

## 版本策略

- **v0.x**: 实验阶段。ABI 不稳定，API 可能变更。
- **v1.0**: 稳定版。ABI 锁定，API 向后兼容。
- **v2.x**: 扩展版。新增 capability、loader、平台。

## Milestone 0: Environment + First Panama Call

**预计时间：** 1-3 天
**状态：** 🔨 In Progress

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 0.1 | 安装 Eclipse Temurin JDK 21 | ⬜ |
| 0.2 | 验证 Rust stable toolchain | ⬜ |
| 0.3 | 创建 monorepo 目录结构 | ⬜ |
| 0.4 | 创建 `runtime-rs/` cdylib crate | ⬜ |
| 0.5 | 实现 `add(a: i32, b: i32) -> i32` | ⬜ |
| 0.6 | 构建 `libmorrow_runtime.so` | ⬜ |
| 0.7 | 创建 `bridge-java/` Gradle project | ⬜ |
| 0.8 | 实现 Panama `Linker.downcallHandle` 调用 `add` | ⬜ |
| 0.9 | 运行并验证输出 `5` | ⬜ |

### 验收标准

```
$ java -cp ... com.morrow.HelloPanama
5
```

---

## Milestone 1: Minimal Runtime

**预计时间：** 1-2 周
**依赖：** M0
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 1.1 | 定义 ABI 数据结构（Handle, error codes） | ⬜ |
| 1.2 | 实现 `morrow_init()` → runtime_handle | ⬜ |
| 1.3 | 实现 `morrow_shutdown(runtime_handle)` | ⬜ |
| 1.4 | 实现 Handle 分配/释放系统 | ⬜ |
| 1.5 | 实现 Error Channel（`morrow_last_error` 等） | ⬜ |
| 1.6 | 实现 Runtime state machine | ⬜ |
| 1.7 | Java 侧：加载 .so + 调用 init/shutdown | ⬜ |
| 1.8 | 泄漏测试：init → shutdown 循环 10 次 | ⬜ |

### 验收标准

```
Test: init → shutdown 10 iterations
  Handle count before: 0
  Iteration 1: init → shutdown → handles: 0 ✓
  Iteration 2: init → shutdown → handles: 0 ✓
  ...
  Iteration 10: init → shutdown → handles: 0 ✓
  Memory: no leak detected
```

---

## Milestone 2: Fabric Integration

**预计时间：** 2-3 周
**依赖：** M1
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 2.1 | 创建 Fabric mod 骨架（`build.gradle` + Loom） | ⬜ |
| 2.2 | 实现 `MorrowMod.onInitialize()` | ⬜ |
| 2.3 | 实现 `NativeLibraryLoader`（平台感知） | ⬜ |
| 2.4 | 实现 `PanamaBridge`（Linker + MethodHandle 管理） | ⬜ |
| 2.5 | 实现 `LifecycleCoordinator`（Fabric → Rust 生命周期同步） | ⬜ |
| 2.6 | 集成 native .so 到 JAR 资源 | ⬜ |
| 2.7 | 在真实 Minecraft 中启动验证 | ⬜ |

### 验收标准

```
[main/INFO] [Morrow]: Native library loaded: libmorrow_runtime.so
[main/INFO] [Morrow]: Panama bridge initialized
[main/INFO] [Morrow]: Runtime initialized (ABI v1, handle=0x1)
[main/INFO] [Morrow]: Ready. Waiting for mods.
```

---

## Milestone 3: Rust Mod Loading

**预计时间：** 2-3 周
**依赖：** M2
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 3.1 | 实现 `.morrow` 包读取（ZIP + manifest 解析） | ⬜ |
| 3.2 | 实现平台 artifact 选择 | ⬜ |
| 3.3 | 实现 `morrow_load_mod(path)` 完整流程 | ⬜ |
| 3.4 | 实现 Mod Registry（注册/查找/卸载） | ⬜ |
| 3.5 | 实现 Mod entry point 调用（`morrow_mod_init`） | ⬜ |
| 3.6 | 编写示例 mod（`hello-morrow`） | ⬜ |
| 3.7 | Mod 加载错误处理与日志 | ⬜ |
| 3.8 | 集成测试：真实 Minecraft + loaded Rust mod | ⬜ |

### 验收标准

```
[main/INFO] [Morrow]: Scanning mods/...
[main/INFO] [Morrow]: Found: mods/hello-morrow.morrow
[main/INFO] [Morrow]: Loading hello-morrow v0.1.0...
[main/INFO] [Morrow]:   Platform: linux-x86_64
[main/INFO] [Morrow]:   Entry: morrow_mod_init
[main/INFO] [Morrow]:   Status: loaded (handle=0x2)
[main/INFO] [hello-morrow]: Hello from Rust!
```

---

## Milestone 4: Event Dispatch

**预计时间：** 2-3 周
**依赖：** M3
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 4.1 | 实现 Event Bus（注册/分发/优先级） | ⬜ |
| 4.2 | 定义标准事件类型（ServerTick, PlayerJoin, etc.） | ⬜ |
| 4.3 | 实现 Fabric event → Rust dispatch | ⬜ |
| 4.4 | 实现 Panama upcall stub（Rust → Java 回调） | ⬜ |
| 4.5 | 实现 `morrow_tick()` 每 tick 驱动 | ⬜ |
| 4.6 | 示例 mod 响应游戏事件 | ⬜ |
| 4.7 | 性能测试：tick dispatch 延迟 | ⬜ |

### 验收标准

```
[Tick 0] [Morrow]: Dispatching to 1 mod(s)
[Tick 20] [hello-morrow]: Second passed! Players: 0
[Tick 40] [hello-morrow]: Second passed! Players: 0
...
[Tick 200] [hello-morrow]: 10 seconds uptime

Benchmark:
  Empty mod tick overhead: <100μs
  1 mod tick overhead: <200μs
  10 mods tick overhead: <500μs
```

---

## Milestone 5: SDK Macros

**预计时间：** 1-2 周
**依赖：** M4
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 5.1 | 实现 `#[morrow::mod_main]` proc macro | ⬜ |
| 5.2 | 实现 Context API（event_bus, commands, config） | ⬜ |
| 5.3 | 实现日志宏（morrow::info!/warn!/error!） | ⬜ |
| 5.4 | 实现事件监听注册糖 | ⬜ |
| 5.5 | 编写 SDK 文档 | ⬜ |
| 5.6 | 编写快速入门指南 | ⬜ |

### 验收标准

```rust
// 5 行代码的 mod
use morrow::prelude::*;

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    morrow::info!("Hello Morrow!");
    Ok(())
}
```

---

## Milestone 6: Linux Verification

**预计时间：** 1 周
**依赖：** M4
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 6.1 | Linux native build 完整流程 | ⬜ |
| 6.2 | Dedicated server 部署测试 | ⬜ |
| 6.3 | 长时间稳定性测试（1 小时） | ⬜ |
| 6.4 | CI: Ubuntu build + test | ⬜ |
| 6.5 | 修复平台特定问题 | ⬜ |

### 验收标准

```
Server uptime: 1 hour
Mods loaded: 1+
Crashes: 0
Memory: stable (no growth trend)
```

---

## Milestone 7: Benchmark Suite

**预计时间：** 1 周
**依赖：** M4
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 7.1 | JNI vs Panama FFM 对比 benchmark | ⬜ |
| 7.2 | Tick overhead 详细测量 | ⬜ |
| 7.3 | Memory footprint 测量 | ⬜ |
| 7.4 | Event dispatch latency 测量 | ⬜ |
| 7.5 | 多 mod 扩展性测试 | ⬜ |

### 验收标准

```
Panama vs JNI:
  call latency:  Panama 5-10ns vs JNI 20-30ns
  tick overhead: Panama <100μs vs JNI <200μs

Memory:
  runtime base:  <2MB
  per mod:      <1MB (empty mod)

Scalability:
  1  mod:   <200μs/tick
  10 mods:  <500μs/tick
  50 mods:  <2ms/tick
```

---

## Milestone 8: v1.0 Release

**预计时间：** 2 周
**依赖：** M5, M6, M7
**状态：** ⬜ Planned

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 8.1 | API 文档（rustdoc） | ⬜ |
| 8.2 | 用户指南（Getting Started） | ⬜ |
| 8.3 | 3+ 示例 mod | ⬜ |
| 8.4 | CI/CD pipeline 完整 | ⬜ |
| 8.5 | 版本号锁定、changelog | ⬜ |
| 8.6 | 发布到 crates.io + GitHub Releases | ⬜ |
| 8.7 | 公告文章 | ⬜ |

### 验收标准

```
morrow = "1.0.0"  # crates.io 可下载
cargo install morrow-cli
morrow new my-first-mod
morrow build
morrow package
# → my-first-mod.morrow 可被 MorrowHost 加载
```

---

## v2 方向（规划中，不实现）

| 功能 | 优先级 | 说明 |
|------|--------|------|
| JDK 25 adapter | 高 | Panama 进一步成熟后的升级 |
| Quilt Loader | 中 | 第二个 loader 支持 |
| NeoForge 支持 | 中 | 如果社区需求大 |
| Plugin system | 中 | Mod 间通信协议 |
| Server extension API | 低 | 管理面板、监控 |
| Hot reload | 低 | 开发迭代体验，技术难度高 |
| macOS (Apple Silicon) | 低 | 等待 Panama 在 macOS 上更稳定 |
| 自研 Loader | 很低 | 摆脱 Fabric，极长期目标 |
| WASM sandbox | 很低 | 替代 native lib 的安全方案 |
