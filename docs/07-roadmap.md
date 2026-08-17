# 07 — 路线图

## 版本策略

- **v0.x**: 实验阶段。ABI 不稳定，API 可能变更。
- **v1.0**: 稳定版。ABI 锁定，API 向后兼容。
- **v2.x**: 扩展版。新增 capability、loader、平台。

## Milestone 0: Environment + First Panama Call

**预计时间：** 1-3 天
**状态：** ✅ Done (2026-08)

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
**状态：** ✅ Done (2026-08)

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
**状态：** ✅ Done (2026-08)，后被 v0.11/v0.12 Mixin 自研 loader 取代
（Fabric 降级为纯类加载器；本表保留为历史计划）

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
**状态：** ✅ Done (2026-08)

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
**状态：** ✅ Done (2026-08)，事件注册糖采用符号发现而非 EventBus
（v0.12 起生产路径为批量派发 morrow_dispatch_batch）

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
**状态：** ✅ Done (2026-08)

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 5.1 | 实现 `#[morrow::mod_main]` proc macro | ✅ |
| 5.2 | 实现 Context API（commands, config, host 调用封装） | ✅ |
| 5.3 | 实现日志宏（morrow::info!/warn!/error!，走 host log） | ✅ |
| 5.4 | 实现事件监听注册糖（`#[morrow::event(kind)]`） | ✅ |
| 5.5 | 编写 SDK 文档 | ✅ |
| 5.6 | 编写快速入门指南（docs/04-sdk-api.md） | ✅ |

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

实现说明:事件注册糖采用 attribute 宏(`#[morrow::event(kind)]` +
普通签名 handler)而非注册式 EventBus — 与符号发现 ABI 匹配;EventBus
(优先级、Arc)留在 v2 规划。额外修复:init panic 穿过 FFI 边界
(catch_unwind 包裹)、`player_death` null cause 的 `read_str` UB、
生产镜像缺 chat-bot.morrow。

---

## Milestone 6: Linux Verification

**预计时间：** 1 周
**依赖：** M5
**状态：** ✅ Done (2026-08)，CI workflow 已随 a2c5e16 推送

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 6.1 | Linux native build 完整流程 | ✅ |
| 6.2 | Dedicated server 部署测试 | ✅ |
| 6.3 | 长时间稳定性测试（1 小时） | ✅ |
| 6.4 | CI: Ubuntu build + test | ✅ |
| 6.5 | 修复平台特定问题 | ✅ |

实现说明:Docker 三阶段构建(cargo → gradle → runtime 镜像)+ loom
runServer 冒烟即为 6.1/6.2 的验证;6.5 在冒烟中暴露并修复了两个
稳定性 bug(事件回调期 RuntimeApi 悬垂指针 → SIGSEGV;EventBuffer
缺 reset → tick 事件丢失),见 v0.15。6.3 实测 55 分钟、tick 65000+、
内存持平(2.244 GiB)。6.4 的 workflow 已就绪待推 GitHub。

v0.16(架构优化,见 design.md 评审结论):
- SDK 状态从 thread_local 升级为全局 static — mod 自 spawn 线程
  也能调用 API;未初始化时显式 panic 而非静默 no-op
- `config::<T: DeserializeOwned>` 类型化 TOML 解析 + `config_raw()`
- runtime 命令注册冲突检测:同名命令拒绝注册(返回错误、不覆盖),
  失败槽位自动归还
- Java 事件缓冲直接写 native MemorySegment(per-tick confined
  arena,design.md §5.1)— 消除 Java heap 往返与 Arena.global() 增长
- 新增真实链路集成测试(testmod cdylib → .morrow 打包 → 加载 →
  派发,不启动 Minecraft);首次运行即抓出命令派发死锁
  (commands 锁跨回调持有,handler 内调 API 重入 data 锁)并修复

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

**性质（v0.16 评审定调）：验收，不是研究。** 性能定位结论见
design.md §零：桥接层已到终点（空 tick 0.04μs，占预算 0.00008%），
M7 的目的是用数据向外部证明承诺、锁定回归基线——不是找瓶颈再优化。

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 7.1 | JNI vs Panama FFM 对比 benchmark（文档化"单价"差距，已知 ~2-3x） | ✅ 实测打平 7.2 vs 7.0ns，纠正早期假设 |
| 7.2 | Tick 全链路测量：EventBuffer 写入 → finish → dispatch_batch → 解析 → 派发 | ✅ 0.393μs/tick（1 事件），0.617μs（8 事件） |
| 7.3 | Memory footprint 测量（运行时基线 + 每 mod 增量 + 每 tick arena 峰值） | ✅ runtime +144KiB，每 mod ~299KiB，shutdown 残留 ~1.5MiB |
| 7.4 | Event dispatch latency 测量（每事件类型，含/不含 catch_unwind） | ✅ 边际 77ns/事件（批量内） |
| 7.5 | 多 mod 扩展性测试（1/10/50 个 no-op mod，验证 O(1) 派发与 mod 数无关） | ✅ 1.399μs@50mods；scalability.rs 进 CI 防二次爆炸 |
| 7.6 | 结果写入 docs/09-benchmarks.md，作为后续性能回归基线 | ✅ |

### 验收标准

```
Panama vs JNI:
  call latency:  实测 7.0 vs 7.2ns（JDK 21 trivial call 两者打平；
  早前"JNI 慢 2-3 倍"的假设不成立，已纠正）

Tick 全链路（空 runtime，1 事件）:
  < 1μs/tick（预算 50ms 的 0.002%）

Scalability（每 mod 一个 tick 回调）:
  1  mod:   < 200μs/tick
  10 mods:  < 500μs/tick
  50 mods:  < 2ms/tick
  增长趋势:   近似线性于 mod 数（每 mod ~μs 级），派发开销本身不随 mod 数放大

Memory:
  runtime base:  < 2MB
  per mod:       < 1MB (empty mod)
```

### 已知缺口（2026-08-17 调查与修复记录）

**独立 agent 模式**（`java -javaagent:morrow.jar -jar server.jar`，无 Fabric）：

| 层 | 缺口 | 状态 |
|----|------|------|
| 打包 | Mixin 类未进 agent jar（`modImplementation` 不打包） | ✅ `agentJar` fat-jar 合并 sponge-mixin + ASM（排除 guava/gson） |
| 宿主服务 | fabric fork 无 vanilla host service（仅 LaunchWrapper/ModLauncher） | ✅ 自研 `MixinServiceVanilla`（IMixinService + IClassProvider + ITransformerProvider + IClassTracker，Knot 同款模式，~200 行）+ `MixinServiceVanillaBootstrap`（检测 Fabric/Forge 存在即让位） |
| 全局属性 | `IGlobalPropertyService` 只列 mojang/modlauncher 的 Blackboard | ✅ `VanillaGlobalPropertyService`（内存 map，仅进 agent jar，不干扰 dev 模式） |
| 验证 | 无真实服务器 e2e | ⚠️ 自动化冒烟已入 CI（build.sh Step 8：premain 启动 Mixin 无 ServiceNotAvailableError）；**真实 vanilla 服务器端到端（MinecraftServer 类实际被转换 + tick 事件流入 Rust）仍需手动验证一次**，步骤：`java -javaagent:morrow.jar -jar server.jar` + 任意 .morrow mod 看日志 |

无 Fabric 时 `Service=Vanilla Env=SERVER` 已实测；Fabric 在场时本服务自动让位（Knot 胜出）已实测。

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
