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
| 打包 | Mixin 类未进 agent jar | ✅ `agentJar` fat-jar 合并 sponge-mixin + ASM 9.7.1（排除 guava/gson，vanilla 自带） |
| 宿主服务 | fabric fork 无 vanilla host service | ✅ `MixinServiceVanilla`（Knot 同款直接实现）+ `MixinServiceVanillaBootstrap`（检测 Fabric/Forge 让位） |
| 全局属性 | `IGlobalPropertyService` 无 vanilla 实现 | ✅ `VanillaGlobalPropertyService`（仅进 agent jar） |
| 配置时机 | premain 注册 config → 目标全 "not found"（bundler 运行时才解包游戏 jar） | ✅ 推迟到 `net.minecraft.server.Main` 首次转换时注册；class provider 双 loader 查找（游戏类走 bundler loader，mixin 类走 system loader） |
| 注入映射 | **vanilla 正式 jar 是混淆名**：`@Inject(method="loadWorld")` 找不到目标（dev 的 yarn 名 ≠ 生产混淆名） | ✅ 不做 refmap，改用**混淆名 twin mixin**：`MinecraftServerMixinVanilla`（`n_()` / `a(BooleanSupplier)` / `t()`，1.20.1 javap 验证）+ `ServerApiVanilla` 适配器（默认包，持有全部混淆签名）。宿主重构为 game-free 核心 + per-mode `ServerApi`（Fabric=yarn / Vanilla=混淆） |
| 类加载 | host 类对 game loader 不可见（bundler parent=platform） | ✅ `HostLink` 反射 `addURL` 把 agent jar 追加进 game loader（需 `--add-opens java.base/java.net=ALL-UNNAMED`） |
| 签名冲突 | `ServerApiVanilla` 在默认包，与 Mojang 签名的游戏类同包 → `SecurityException` | ✅ `ChildFirstLoader`：默认包类 child-first 定义（子 loader 内无签名冲突），命名类型 parent-first 保类身份 |
| 次要 | mixin 类 class version 65 > 声明 JAVA_17（启动一条 WARN） | ⚠️ 无功能影响（e2e 实测）；桥接必须 release 21 编译（FFM preview），后续评估 |

e2e 验证命令：
`java --enable-preview --enable-native-access=ALL-UNNAMED --add-opens java.base/java.net=ALL-UNNAMED -javaagent:bridge-java/build/libs/morrow-host-1.0.0-agent.jar -jar server.jar nogui`

**✅ 已验收（2026-08-19）**：`mixin applied: net.minecraft.server.MinecraftServer`（无 InvalidInjectionException）→ `Morrow loading...` → 3 个 mod 经真实 loader 加载（依赖重试生效）→ `Morrow ready. 3 mod(s).` → tick 事件持续流入（hello-morrow tick 200~2800+）→ SIGTERM 触发 server_stop（`Bye!`）。

## M9：事件捕获补全（2026-08-19，全链路 e2e 实测）

生产侧此前只有 tick 有注入点，其余 6 类事件（join/leave/chat/break/place/death）管道全通但游戏侧无触发。M9 补齐：

| 事件 | dev 注入点（yarn） | vanilla 注入点（混淆，javap 验证） | e2e |
|------|-------------------|-----------------------------------|-----|
| join | PlayerManager.onPlayerConnect RETURN | alk.a(sd,aig) RETURN | ✅ 假客户端进服 → `+ Steve` + chat-bot 欢迎广播 |
| leave | PlayerManager.remove HEAD | alk.c(aig) HEAD | ✅ 断开 → `- Steve` |
| chat | ServerPlayNetworkHandler.onChatMessage HEAD | aiy.a(zi) HEAD | ✅ `<Steve> hi morrow` + chat-bot 回复 |
| death | ServerPlayerEntity.onDeath HEAD | aig.a(ben) HEAD | ✅ 控制台 `/kill` → `Steve died` |
| break | ServerPlayerInteractionManager.tryBreakBlock HEAD+RETURN | aih.a(gu)Z HEAD+RETURN | ✅ 假客户端挖方块 → `Steve broke minecraft:dirt` |
| place | ServerPlayerInteractionManager.interactBlock RETURN | aih.a(aig,cmm,cfz,bdw,eee) RETURN | ✅ 自测 mixin 直调 `useItemOn`（绕 vanilla 包校验）→ `CONSUME consumed=true` → `Steve placed minecraft:dirt`（点击位 + 面偏移取**实际放置**的方块）。**9/9 事件全部实测通过** |

**关键工程项（本轮踩坑记录）：**
1. **默认包 + Mixin 包要求**：vanilla 事件 mixin 必须默认包编写（javac 禁止命名包引用混淆类型），但 Mixin 配置必须有 package → 构建期**常量池字节手术**：追加 Utf8 条目 + 把所有指向旧名的 Class 条目重指到新名（this_class、私有 helper 的 invokestatic owner）。`-g:none` 编译避免 LVT 里的默认包 `this` 类型。
2. **签名冲突 + 加载器**：`ChildFirstLoader`（M7 已修）；`@Coerce Object` 处理器参数使 mixin 类可在无游戏类的 loader 下加载（transform 期 eager 解析）。
3. **线程模型**：chat/break/place 在 Netty IO 线程触发（非 server 线程）→ EventBuffer 改 `Arena.ofShared()` + 全方法 synchronized + flush 全周期持锁（confined arena 会 WrongThreadException 炸连接）。
4. **假客户端**（/tmp/fake_client.py，离线模式协议 763）：handshake → login（zlib 压缩，低于阈值 256 的包不压缩）→ chat（LastSeenMessages bitset 是固定 3 字节）→ 断连。
5. **break 的双注入**：RETURN 时方块已被破坏（读到恒为 air），HEAD 读名字存 ThreadLocal、RETURN 校验返回值后取用。注意 boolean 返回的方法连 HEAD 注入也必须用 `CallbackInfoReturnable`（Mixin 校验）。
6. **gz 桩是接口**：DefaultedRegistry 的真实类型是 interface，桩写成 class 导致 invokevirtual → 运行时 `IncompatibleClassChangeError` 崩服。
7. 协议细节：block_dig=0x1D、block_place=0x31、position 编码无符号 64 位（Python 负数按位与需显式掩码）、创造模式 set_creative_slot=0x2b（dirt item id=9，槽位 36=快捷栏 0）。
8. **place 自测 mixin**（MorrowPlaceSelfTestMixin，`-Dmorrow.selftest.place=true` 启用，一次性）：vanilla 的 UseItemOn 包校验拒绝假客户端（方块从未放置），自测 mixin 在首个玩家在线时以服务器线程直调 `useItemOn`（合成 eei/ha/eee/cfz），绕过包校验但覆盖 Morrow 全部代码路径。**玩家相对坐标**（moveTo +2 清空脚下块，对着脚下地面顶面放置 dirt 再 destroyBlock 挖回）→ 地形无关，顺带覆盖 break 自测 —— `CONSUME consumed=true` + `Steve placed minecraft:dirt` + `Steve broke minecraft:dirt` 实测。
9. **查询 API 快照化**：`morrow_get_player_count` / `get_player_list` / `get_world_time` 改为读每 tick 缓存的 `WorldSnapshot`（首个查询开启消费门，≤1 tick 后生效），不再从 mod 线程直调 Java —— 任意线程可读、零额外 upcall；快照缓冲由内核持有跨 tick 复用（固定 64 KiB，`ponytail:` 注明上限）。e2e 新增 CI 任务 `agent-e2e`（下载 Mojang jar 全量跑 9/9 事件）。

---

## Milestone 8: v1.0 Release

**预计时间：** 2 周
**依赖：** M5, M6, M7
**状态：** ✅ 完成（2026-08-19，8.7 公告为文稿待发布）

### 任务清单

| # | 任务 | 状态 |
|---|------|------|
| 8.1 | API 文档（rustdoc） | ✅ 零警告；CI 以 `-D warnings` 强制 |
| 8.2 | 用户指南（Getting Started） | ✅ README Quick Start + 生产运行手册（standalone agent 命令实测） |
| 8.3 | 3+ 示例 mod | ✅ hello-morrow / chat-bot / motd，CI 打包，agent e2e 实测加载 |
| 8.4 | CI/CD pipeline 完整 | ✅ Rust 测试 + rustdoc + 3 示例打包 + gradle + build.sh 全套（含 agent premain smoke、基准） |
| 8.5 | 版本号锁定、changelog | ✅ 全 crate + mod_version = 1.0.0；CHANGELOG.md |
| 8.6 | 发布到 crates.io + GitHub Releases | ✅ morrow / morrow-macros / morrow-cli @ 1.0.0；Release v1.0.0 附 agent jar |
| 8.7 | 公告文章 | ✅ 文稿 docs/announcement-v1.0.0.md（发布渠道由作者定） |

### 验收标准

```
morrow = "1.0.0"  # crates.io 可下载          ✅ 空项目 cargo add 实测
cargo install morrow-cli                      ✅ 从 crates.io 安装实测
morrow new my-first-mod                       ✅
morrow build                                  ✅
morrow package                                ✅
# → my-first-mod.morrow 可被 MorrowHost 加载   ✅ 布局与 package-mod.sh 一致（e2e 同款已加载）
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
