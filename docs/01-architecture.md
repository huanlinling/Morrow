# 01 — 完整架构（v0.16）

## 架构全景图

```
java -javaagent / -jar server.jar + morrow.jar（drop 进 mods/）
  │
  ├─ Fabric Loader 仅作为类加载器（Morrow 不依赖 Fabric API）
  │     └─ Mixin → MinecraftServerMixin（唯一的注入点）
  │           ├─ loadWorld RETURN  → MorrowMod.init()
  │           ├─ tick HEAD        → MorrowMod.onTick()  → EventBuffer 累积
  │           ├─ tick RETURN      → MorrowMod.flushBatch() → 1 次 downcall/tick
  │           └─ shutdown HEAD    → MorrowMod.onShutdown()
  │
  └─ Panama FFM（JDK 21 java.lang.foreign）
        ├─ Downcalls：morrow_init / morrow_load_mod / morrow_dispatch_batch /
        │            morrow_dispatch_server_start|stop / morrow_register_host_api /
        │            morrow_dispatch_command / morrow_shutdown
        └─ HostVtable（7 个 upcall stub，Arena.global() 分配 56 字节）
             get_player_count / send_message / get_player_list /
             execute_command / get_world_time / log_message / get_world_snapshot
        │
        └─ Rust Runtime（libmorrow_runtime.so）
             ├─ 每个 extern "C" 入口包 catch_unwind（panic 绝不穿透 FFI）
             ├─ RuntimeKernel：单把 Mutex<RuntimeData> 装全部状态
             │    registry / dispatch(Arc) / lifecycle / errors / host_api /
             │    commands / configs / snapshot
             ├─ morrow_dispatch_batch：零拷贝解析 wire format →
             │    Arc 快照回调表（1 次引用计数，不克隆 map）→ 锁外逐个
             │    dispatch（每个回调独立 catch_unwind）
             ├─ WorldSnapshot：消费者门禁——读 API（player_count/list/time）
             │    首次查询开启每 tick 刷新，之后零 upcall、任何线程安全
             └─ Mod A, B, C...（dlopen 的 cdylib，符号发现注册回调）
```

## 分层详解

### Layer 1: Java Host（bridge-java）

```
src/main/java/com/morrow/
├── agent/MorrowAgent.java          # premain：注册 Mixin
│   ├── HostLink.java               # addURL 让 game loader 看到 agent 类（agent 模式）
│   └── ChildFirstLoader.java       # 默认包 child-first（绕 Mojang 签名冲突）
├── mixin/MinecraftServerMixin.java # 唯一注入点：loadWorld / tick / shutdown（dev，yarn 名）
└── host/
    ├── MorrowMod.java              # 宿主逻辑：init / onTick / flushBatch / onShutdown（game-free）
    ├── ServerApi.java              # 游戏访问接口（适配器模式的缝）
    ├── ServerApiFabric.java        # yarn 名实现（dev/loom）
    ├── PanamaBridge.java           # 一次性 SymbolLookup + downcall MethodHandle
    ├── NativeLibraryLoader.java    # 平台感知加载 libmorrow_runtime.so
    └── EventBuffer.java            # 每 tick 事件累积 → off-heap MemorySegment
src/vanilla/java/                   # agent 模式专用（混淆名，只进 agent jar）
    ├── ServerApiVanilla.java       # 默认包适配器（1.20.1 javap 验证的混淆签名）
    └── com/morrow/mixin/MinecraftServerMixinVanilla.java  # 混淆名 twin mixin
```

**启动序列（MorrowMod.init，由 Mixin 在 loadWorld RETURN 触发）：**
1. 加载 native runtime → 2. 建 Panama bridge → 3. `morrow_init(ABI_VERSION)`
→ 4. 扫描 `mods/*.morrow` 逐个 `morrow_load_mod`（失败的包重试一轮，给依赖排序）→
5. 绑定 `morrow_dispatch_batch` → 6. `morrow_dispatch_server_start` →
7. 注册 HostVtable（7 个 upcall stub）。

**为什么是 Agent + Mixin 而不是 Fabric API：** v0.11 前 Morrow 是 Fabric
ModInitializer；v0.12 起改为 Mixin 直接注入 `MinecraftServer`，Fabric 只当
类加载器用。收益：不依赖 Fabric API 版本链，`morrow.jar` 一个包即装即用。

### Layer 2: Panama Bridge

- **Downcall**：`Linker.downcallHandle` + `invokeExact`，实测 9.3-9.7ns/次。
- **Upcall**：7 个 `upcallStub` 的地址写进 56 字节 vtable，`morrow_register_host_api`
  一次性传给 Rust（Rust 侧按 `HostVtable` `#[repr(C)]` 布局读取）。
- **内存**：`Arena.global()` 只用于长生命周期对象（vtable、upcall stub）；
  事件数据用 EventBuffer 的 per-tick `Arena.ofConfined()`，dispatch 后 close。

### Layer 3: Rust Runtime（runtime-rs）

```
src/
├── lib.rs            # 全部 extern "C" 导出 + 批量解析/dispatch 主循环
├── abi/
│   ├── mod.rs        # ABI 版本、结果码、is_abi_compatible
│   └── handles.rs    # Handle(u64) + HandleTable<T>（Arc 注册表）
├── runtime/
│   ├── mod.rs        # RuntimeKernel：Mutex<RuntimeData> + 状态机
│   └── state.rs      # Ready / ShuttingDown / Dead
├── host_api.rs       # HostVtable、WorldSnapshot、CommandRegistry、Quarantine
├── event/tick.rs     # TickRegistry（mod 名 → fn 指针）
├── mod_loader/       # .morrow ZIP 解析、manifest、平台 artifact、dlopen
├── panic.rs          # ffi_boundary：每个 FFI 入口的 catch_unwind 包装
├── error.rs          # ErrorChannel
└── logger.rs
```

**并发模型（v0.16 固化）：**
- 全部每-runtime 状态在**一把** `Mutex<RuntimeData>` 里（旧版是 8 个全局 map）。
- 铁律：**锁内收集（clone fn 指针表）、锁外调用**。mod 回调可以重入任意
  Runtime API 而不死锁。
- 每个 mod 回调独立 `catch_unwind`；panic 的 mod 进 Quarantine，后续 tick
  跳过它，其他 mod 与服务器继续运行。
- 写操作（send_message / execute_command）跨线程安全：非主线程调用进
  outbound 队列，下一 tick 主线程 flush（≤50ms）；主线程调用直达，零延迟。
- 读操作（player_count/list/time）快照支撑：查询自动开启每 tick 世界
  快照刷新（消费者门禁），数据 ≤1 tick 滞后、任何线程安全、零 upcall；
  首次刷新落地前返回空值。

### Layer 4: SDK（sdk-rs + morrow-macros）

- `#[morrow::mod_main]` 生成 `morrow_mod_init` 导出符号。
- `#[morrow::event(kind)]`（tick/join/leave/chat/break/place/death/start/stop）
  生成对应的 `morrow_mod_*` 导出符号——与运行时的符号发现 ABI 匹配。
- 全局 API（`send_message` / `player_count` / `config::<T>` ...）走 global
  static vtable，mod 自 spawn 的线程也可用；未初始化时显式 panic。
- `read_str` 零拷贝借用事件缓冲区，无每事件分配。

## 热路径数据流（每 tick）

```
tick HEAD   EventBuffer.tick(n)：14 字节写入（6 字节头 + u64 tick 号）
期间        玩家 join/chat 等事件同样追加进 buffer（二进制，无 Java 堆中转）
tick RETURN flushBatch()：
               finish() 盖总数 → 1 次 downcall morrow_dispatch_batch(ptr, len)
Rust 侧：
  1. 解析 u32 count + 逐事件（u16 type + u16 len1 + u16 len2 + payload）
  2. 锁内 Arc 快照：dispatch 表 + host_api（1 次引用计数，无 map 克隆）
  3. WorldSnapshot 有消费者才 1 次 upcall 刷新（读 API 首次查询即开启）
  4. 锁外逐回调 dispatch，每个 catch_unwind；panic → quarantine
最后        eventBuffer.reset()：close per-tick arena（内存即时归还）
```

**为什么快（性能定位见 design.md §零）：**
- FFI 边界穿越：每 tick **1 次**，与 mod 数、事件数无关（O(1)）。
- mod 扇出在 native 侧以 fn 指针完成（~1ns），不跨边界。
- 分配：每 tick 1 个 arena；事件数据零拷贝解析。
- 代价：事件最多延迟 1 tick（50ms）送达——MC 是 20 TPS 快照模型，免费。

## 延迟预算（实测，docs/09-benchmarks.md）

| 项 | 实测 |
|---|---|
| Panama downcall | 9.3-9.7 ns/次 |
| 空 runtime 单 tick 派发 | 0.04 μs（占 50ms 预算的 0.00008%） |
| 理论 TPS 上限 | ~2,300-2,700 万/秒 |
| 运行时内存 | .so ~2.2MB + 内核 ~1KB + 每 mod ~4KB |

**结论：loader 开销不是瓶颈，mod 代码才是。桥接层不再投入优化。**
