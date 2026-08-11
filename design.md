# Morrow Design Plan

> 构建基于 **Rust + Project Panama** 的现代 Minecraft Native Runtime Platform
>
> 版本: 0.1.0-draft | 日期: 2026-08-11 | 状态: 设计阶段

---

## 零、定位声明

**Morrow 是什么：**

| ✅ 是 | ❌ 不是 |
|-------|--------|
| Minecraft 原生运行时平台 | Rust JNI Demo |
| Rust 编写 Mod 的完整 SDK | Fabric 的小插件 |
| 与 Java Mod 共存的桥梁 | 仅调用 native 的实验品 |
| 面向性能的严肃基础设施 | 玩具项目 |

**Morrow 提供：**
- Rust 编写 Mod（proc macro SDK）
- Java Host 桥接（Fabric Mod → Panama FFI）
- 跨平台 Native Artifact 加载（`.morrow` 包格式）
- 稳定 ABI（opaque handles, no struct exposure）
- Runtime 生命周期管理
- Host Capability 抽象（registry, events, 未来扩展）
- Panic 隔离与错误边界

---

## 一、技术选型与约束

### 1.1 版本矩阵（锁定）

| Component | Version | Why |
|-----------|---------|-----|
| Java | Eclipse Temurin JDK 21 | Panama FFM API 正式稳定 (since JDK 22) |
| Minecraft | 1.20.1 | 生态成熟、资料丰富、社区稳定 |
| Fabric Loader | >= 0.16.x stable | 轻量、注入点清晰、文档好 |
| Rust | stable (>= 1.80) | 不做 nightly 依赖 |
| OS (dev) | Windows 11 / Linux | 主力开发环境 |
| OS (CI) | Linux | 通过 Docker 环境，与部署目标一致 |
| Build (Java) | Gradle 8.x + Loom | Fabric 标准构建工具链 |
| Build (Rust) | Cargo workspace | Rust 标准构建系统 |
| Dev Env | Docker (eclipse-temurin:21-jdk) | 完全隔离可复现的开发环境 |

### 1.2 首版不支持（明确排除）

- ❌ Java 8 / 17（Panama 不可用或预览）
- ❌ Forge / NeoForge（v1 只做 Fabric；架构预留扩展点）
- ❌ 热重载（v2 谨慎探索）
- ❌ macOS（Apple Silicon 下的 Panama + native lib 坑太多，v2 再搞）
- ❌ 插件系统（先稳住核心，别过早抽象）
- ❌ 自研 Loader（先用 Fabric，验证完再说）

### 1.3 为什么是 Panama 而不是 JNI？

| 维度 | JNI | Panama FFM |
|------|-----|------------|
| 调用开销 | ~20-30ns | ~5-10ns（inline 可达 0） |
| 内存管理 | 手动 GlobalRef/LocalRef 管理 | Arena 作用域管理，自动释放 |
| 类型安全 | JNI 类型系统老旧 | ValueLayout 类型安全 |
| 代码量 | Java + C glue code | 纯 Java，直接 downcall |
| 运行时 | 需要加载 JNI 库 | JDK 内置，无需额外 Runtime |
| 未来 | 遗留 API | JDK 官方主推方向 |

**结论：Panama 是现代 JDK 的 native 互操作标准答案。Morrow 全栈使用 Panama FFM API。**

---

## 二、整体架构（优化版）

```
┌────────────────────────────────────────────┐
│             Minecraft (Java)               │
├────────────────────────────────────────────┤
│         Fabric Host Adapter (Java)         │
│  ┌──────────────────────────────────────┐  │
│  │  Fabric Mod Bootstrap                │  │
│  │  Lifecycle Hooks                     │  │
│  │  Native Library Discovery & Loading  │  │
│  │  Event → Rust Dispatch               │  │
│  │  Capability Negotiation              │  │
│  └──────────────┬───────────────────────┘  │
│                 │ Panama FFM API             │
│  ┌──────────────▼───────────────────────┐  │
│  │  Panama Linker                       │  │
│  │  MemorySegment / Arena management    │  │
│  │  Downcall / Upcall stubs             │  │
│  └──────────────┬───────────────────────┘  │
├─────────────────┼───────────────────────────┤
│                 │  FFI Boundary             │
├─────────────────┼───────────────────────────┤
│  ┌──────────────▼───────────────────────┐  │
│  │  Morrow Runtime Core (Rust cdylib)   │  │
│  │                                      │  │
│  │  ┌────────────────────────────────┐  │  │
│  │  │ ABI Layer (extern "C" fns)     │  │  │
│  │  └──────────┬─────────────────────┘  │  │
│  │  ┌──────────▼─────────────────────┐  │  │
│  │  │ Runtime Kernel                 │  │  │
│  │  │  ├─ Mod Registry               │  │  │
│  │  │  ├─ Event Bus                  │  │  │
│  │  │  ├─ Lifecycle Scheduler        │  │  │
│  │  │  └─ Capability Registry        │  │  │
│  │  └──────────┬─────────────────────┘  │  │
│  │  ┌──────────▼─────────────────────┐  │  │
│  │  │ Panic Boundary (catch_unwind)  │  │  │
│  │  └──────────┬─────────────────────┘  │  │
│  │             │                         │  │
│  │   ┌─────────┼─────────┐              │  │
│  │   ▼         ▼         ▼              │  │
│  │ Mod A    Mod B    Mod C              │  │
│  └──────────────────────────────────────┘  │
│                                            │
│  Morrow Runtime (.so / .dll)               │
└────────────────────────────────────────────┘
```

### 2.1 分层职责

#### Layer 1: Fabric Host Adapter（Java）

```
bridge-java/
├── fabric-host/
│   ├── src/main/java/com/morrow/host/
│   │   ├── MorrowMod.java          # Fabric ModInitializer entry
│   │   ├── NativeLibraryLoader.java # Platform-aware .so/.dll loading
│   │   ├── PanamaBridge.java        # Linker setup, downcall/upcall
│   │   ├── LifecycleCoordinator.java # JVM lifecycle → Rust lifecycle
│   │   ├── EventDispatcher.java     # Fabric events → Rust dispatch
│   │   └── CapabilityChannel.java   # Capability negotiation protocol
│   └── build.gradle
```

职责：
- Fabric `ModInitializer` 入口
- 识别平台（os.name, os.arch）→ 选择正确的 native artifact
- 通过 Panama `SymbolLookup` 加载 `libmorrow_runtime.so`
- 绑定 Rust 导出的 extern "C" 函数为 MethodHandle
- 将 Fabric 生命周期事件翻译为 Rust dispatch 调用
- 提供 Arena 管理（每个 tick 一个 Arena scope）

#### Layer 2: Panama Bridge（JDK FFM API）

核心 API 使用：

```java
// 1. 获取系统链接器
Linker linker = Linker.nativeLinker();

// 2. 加载 native library
SymbolLookup lookup = SymbolLookup.libraryLookup(
    Path.of("libmorrow_runtime.so"), Arena.global());

// 3. 查找函数符号
MemorySegment addr = lookup.find("morrow_init")
    .orElseThrow(() -> new UnsatisfiedLinkError("morrow_init not found"));

// 4. 创建 downcall handle
MethodHandle morrow_init = linker.downcallHandle(addr,
    FunctionDescriptor.of(ValueLayout.JAVA_INT)); // () -> int32

// 5. 调用（每个 tick 在 arena scope 内）
try (Arena arena = Arena.ofConfined()) {
    int result = (int) morrow_init.invokeExact();
}
```

**关键设计决策：**
- 使用 `Arena.ofConfined()` 作为 tick-local scope，确保每个 tick 的 native memory 在 tick 结束时自动释放
- `Arena.global()` 仅用于长生命周期对象（如 runtime state handle）
- 所有跨 FFI 的字符串通过 `MemorySegment` + UTF-8 传递，不在 native 侧分配 Java String
- Upcall stub 用于 Rust → Java 回调（事件通知）

#### Layer 3: Morrow Runtime Core（Rust cdylib）

```
runtime-rs/
├── Cargo.toml          # [lib] crate-type = ["cdylib"]
├── src/
│   ├── lib.rs           # Root: export extern "C" functions
│   ├── abi/
│   │   ├── mod.rs       # ABI type definitions
│   │   ├── handles.rs   # Opaque handle management
│   │   └── layout.rs    # Memory layout contracts
│   ├── runtime/
│   │   ├── mod.rs       # Runtime kernel
│   │   ├── registry.rs  # Mod registry (HashMap<ModId, ModState>)
│   │   └── scheduler.rs # Lifecycle phase scheduler
│   ├── event/
│   │   ├── mod.rs       # Event bus core
│   │   └── dispatch.rs  # Type-erased event dispatch
│   ├── panic.rs         # catch_unwind boundary
│   ├── platform/
│   │   ├── mod.rs       # Platform abstraction
│   │   ├── windows.rs
│   │   └── linux.rs
│   └── cap/
│       ├── mod.rs       # Capability negotiation
│       └── types.rs     # Capability descriptors
```

#### Layer 4: SDK（Rust crate for mod developers）

```
sdk-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── context.rs   # Context: capability accessor
│   ├── event.rs     # Event listener trait
│   └── mod_main.rs  # #[morrow::mod_main] proc macro re-export
├── morrow-macros/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs   # proc macro implementation
```

---

## 三、ABI 设计（优化版）

### 3.1 核心原则

1. **不暴露 Rust struct layout** — 所有跨 FFI 的数据通过 opaque handle (`u64`)
2. **不跨 FFI unwind** — Rust panic 绝不穿透 FFI 边界
3. **allocator 不跨边界** — Rust 分配的由 Rust 释放，Java 分配的由 Java 释放
4. **字符串用 arena** — 所有字符串在指定 arena 内分配，生命周期明确
5. **版本协商前置** — `morrow_init` 时交换 ABI 版本号

### 3.2 导出函数清单

```c
// ──── Runtime Lifecycle ────

// 初始化 runtime，返回 runtime handle
// 参数：abi_version (输入), arena (用于返回数据的临时内存)
// 返回：runtime_handle (u64 opaque), 0 表示失败
uint64_t morrow_init(uint32_t abi_version, uint64_t arena_handle);

// 关闭 runtime
// 参数：runtime_handle
// 返回：0 成功，非零错误码
uint32_t morrow_shutdown(uint64_t runtime_handle);

// ──── Mod Management ────

// 加载一个 mod（从 .morrow 包路径）
// 返回：mod_handle (u64 opaque), 0 表示失败
uint64_t morrow_load_mod(uint64_t runtime_handle,
                          uint64_t mod_path_str_ptr,
                          uint32_t mod_path_str_len);

// ──── Event Dispatch ────

// 分发事件到所有已注册 mod
// event_data: JSON 或二进制 encoded event
// 返回：处理该事件的 mod 数量
uint32_t morrow_dispatch_event(uint64_t runtime_handle,
                                uint64_t event_type_str_ptr,
                                uint32_t event_type_str_len,
                                uint64_t event_data_ptr,
                                uint32_t event_data_len);

// ──── Tick ────

// 每 tick 调用，驱动 mod 的 on_tick
void morrow_tick(uint64_t runtime_handle);

// ──── Error Channel ────

// 获取最后一个错误（panic / error 信息）
// 返回 error_handle，0 表示无错误
uint64_t morrow_last_error(uint64_t runtime_handle);

// 读取 error 详情
// 将 error message 写入 buffer
uint32_t morrow_error_message(uint64_t error_handle,
                               uint64_t buffer_ptr,
                               uint32_t buffer_capacity);
```

### 3.3 Opaque Handle 系统

所有跨 FFI 的 Rust 对象通过 opaque handle 引用：

```rust
// 内部：handle → Rust object 的映射
// 外部：Java 侧只看到 u64

pub type Handle = u64;

// Runtime kernel
static RUNTIME_REGISTRY: LazyLock<Mutex<HashMap<Handle, Runtime>>> = ...;

// 分配 handle
fn allocate_handle<T>(obj: T, registry: &Mutex<HashMap<Handle, T>>) -> Handle {
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
    registry.lock().insert(handle, obj);
    handle
}
```

**为什么是 u64 而不是指针？**
- 指针暴露了内存布局，违反 ABi 稳定性
- u64 可以编码更多信息（如 generation 防止 use-after-free）
- 架构无关（32/64 bit 兼容）

### 3.4 错误处理协议

```
┌─────────┐         ┌─────────┐
│  Rust   │ panic!  │  Panic  │
│  Mod    │────────▶│ Boundary │
└─────────┘         └────┬────┘
                          │ catch_unwind
                          ▼
                    ┌─────────┐
                    │  Error  │
                    │ Channel │
                    └────┬────┘
                          │ morrow_last_error()
                          ▼
                    ┌─────────┐
                    │  Java   │
                    │  Host   │
                    └─────────┘
```

- Rust panic → `catch_unwind` → Error Channel → Java host 轮询
- Java 异常（在 upcall 中）→ 转为 error code 返回 Rust
- 错误始终 **不跨 FFI 传播为异常**，而是走带内 error channel

---

## 四、生命周期模型（优化版）

### 4.1 Runtime 状态机

```
                    ┌──────────┐
                    │  Created │
                    └────┬─────┘
                         │ morrow_init()
                         ▼
                    ┌──────────┐
                    │   Init   │─── panic → Error Channel
                    └────┬─────┘
                         │ all mods loaded
                         ▼
                    ┌──────────┐
              ┌────▶│  Ready   │
              │     └────┬─────┘
              │          │ morrow_tick() (每 tick)
              │          ▼
              │     ┌──────────┐
              │     │  Ticking │─── mod panic → quarantine mod
              │     └────┬─────┘     continue runtime
              │          │
              │          ▼
              │     ┌──────────┐
              │     │  Ready   │───────▶ next tick
              │     └──────────┘
              │
              │     morrow_shutdown()
              │          │
              │          ▼
              │     ┌──────────┐
              └─────│ShuttingDn│
                    └────┬─────┘
                         │ all mods unloaded
                         ▼
                    ┌──────────┐
                    │   Dead   │
                    └──────────┘
```

### 4.2 Mod 生命周期 Hooks

```rust
pub trait ModLifecycle {
    /// 模组加载时调用（注册事件监听器等）
    fn on_init(&mut self, ctx: &mut Context) -> Result<(), MorrowError>;

    /// 服务端启动完成
    fn on_server_start(&mut self, ctx: &mut Context) -> Result<(), MorrowError>;

    /// 每游戏 tick（20 TPS = 每 50ms）
    fn on_tick(&mut self, ctx: &mut Context, tick: u64) -> Result<(), MorrowError>;

    /// 服务端关闭
    fn on_server_stop(&mut self, ctx: &mut Context) -> Result<(), MorrowError>;

    /// 模组卸载
    fn on_shutdown(&mut self, ctx: &mut Context) -> Result<(), MorrowError>;
}
```

### 4.3 加载时序

```
JVM Start
  │
  ▼
Fabric Loader init
  │
  ▼
MorrowHostMod.onInitialize()
  │
  ├─ 1. Platform detection (os/arch)
  ├─ 2. NativeLibraryLoader.load("morrow_runtime")
  ├─ 3. PanamaBridge.setup()
  │     ├─ SymbolLookup → find exported fns
  │     └─ Create downcall MethodHandles
  ├─ 4. morrow_init(ABI_VERSION, arena)
  │     └─ Runtime kernel initialized
  ├─ 5. Scan mods/ directory for .morrow packages
  ├─ 6. For each .morrow:
  │     ├─ Parse manifest.toml
  │     ├─ Platform artifact selection
  │     └─ morrow_load_mod(path)
  ├─ 7. Dispatch on_server_start
  │
  ▼
Game Loop
  │
  ├─ Tick 0 ... N
  │   └─ morrow_tick(runtime_handle)
  │
  ▼
Server Stop
  │
  └─ morrow_shutdown(runtime_handle)
```

---

## 五、内存管理（优化版）

### 5.1 Arena 分层

```
┌─────────────────────────────────┐
│   Arena.global() (Java)         │  ← Runtime handle, global lookup tables
│   生命周期：JVM 整个生命周期      │
├─────────────────────────────────┤
│   Arena.ofConfined() per tick   │  ← Tick-scoped event data
│   生命周期：单个 Minecraft tick   │     morrow_tick() 调用期间
├─────────────────────────────────┤
│   Arena.ofConfined() per event  │  ← 单次 event dispatch 的临时数据
│   生命周期：单次 dispatch 调用    │
└─────────────────────────────────┘
```

### 5.2 所有权规则

| 数据 | 分配方 | 释放方 | 机制 |
|------|--------|--------|------|
| Runtime state | Rust | Rust (morrow_shutdown) | Rust ownership |
| Mod instances | Rust (via dlopen) | Rust (morrow_shutdown) | Rust ownership |
| Event data | Java (Arena) | Java (Arena.close) | Arena scope |
| Strings (to Rust) | Java (Arena) | Java (Arena.close) | Arena scope, Rust 只读 |
| Strings (to Java) | Rust (Arena) | Java (Arena.close) | 写入 Java 提供的 buffer |
| Opaque handles | Rust | Rust | Generation-based invalidation |

### 5.3 零拷贝路径

对于高频数据（如 tick position），设计零拷贝路径：

```rust
// Java 侧：分配 off-heap MemorySegment
// Rust 侧：通过 pointer 直接读写，无需序列化

// Java:
try (Arena arena = Arena.ofConfined()) {
    MemorySegment pos = arena.allocate(24); // x: f64, y: f64, z: f64
    pos.set(ValueLayout.JAVA_DOUBLE, 0, x);
    pos.set(ValueLayout.JAVA_DOUBLE, 8, y);
    pos.set(ValueLayout.JAVA_DOUBLE, 16, z);
    morrow_tick_position(runtime_handle, pos); // pass segment to Rust
}

// Rust:
#[unsafe(no_mangle)]
pub extern "C" fn morrow_tick_position(
    runtime_handle: u64,
    pos_ptr: *const f64, // Direct pointer to Java off-heap memory
) {
    // SAFETY: 指针在 Arena 生命周期内有效
    let pos = unsafe { std::slice::from_raw_parts(pos_ptr, 3) };
    // use pos[0], pos[1], pos[2] directly — no copy, no allocation
}
```

---

## 六、包格式（优化版）

### 6.1 `.morrow` 包结构

```
example-mod.morrow  (ZIP compressed, store method for speed)
│
├── manifest.toml           # 包元数据
├── windows-x86_64/
│   └── mod.dll
├── linux-x86_64/
│   └── libmod.so
├── linux-aarch64/          # 预留 ARM 服务器
│   └── libmod.so
└── assets/                 # 可选：模组资源文件
    ├── textures/
    └── sounds/
```

### 6.2 manifest.toml

```toml
[package]
name = "example-mod"
version = "0.1.0"
description = "An example Morrow mod"
authors = ["dev <dev@example.com>"]
license = "MIT"

[morrow]
api_version = 1
min_runtime = "0.1.0"

[minecraft]
version = ">=1.20.1, <1.22"
loader = "fabric"

[entry]
symbol = "morrow_mod_init"  # Rust extern "C" entry point

[dependencies]  # 可选：依赖其他 Morrow mod
# other-mod = ">=1.0.0"
```

### 6.3 加载流程

```
1. Java host 扫描 mods/*.morrow
2. 对每个包：
   a. 解压 manifest.toml (ZIP 随机访问)
   b. 解析 manifest → 验证 api_version 兼容
   c. 选择平台 artifact (os.name + os.arch → 路径)
   d. 提取 .so/.dll 到临时目录（或直接用 ZIP 中的路径）
   e. System.load(tempPath) 或者 dlopen
   f. 调用 entry symbol 注册 mod
```

---

## 七、Panic 隔离（优化版）

### 7.1 隔离层级

```
Layer 1: Mod panic
  ├─ catch_unwind at mod dispatch boundary
  ├─ 隔离到单个 mod — 其他 mod 继续运行
  └─ error → Java log + error channel

Layer 2: Runtime panic
  ├─ catch_unwind at morrow_dispatch_event boundary
  ├─ 整个 runtime 进入 degraded mode
  └─ error → Java log + attempt recovery

Layer 3: Crash (unrecoverable)
  ├─ SIGSEGV / stack overflow
  ├─ 进程级崩溃
  └─ Java 侧检测 native crash → graceful server shutdown
```

### 7.2 Rust 侧实现

```rust
pub fn dispatch_to_mod(
    mod_handle: Handle,
    event: &Event,
) -> Result<(), ModError> {
    let registry = MOD_REGISTRY.lock();
    let mod_state = registry.get(&mod_handle)
        .ok_or(ModError::NotFound)?;

    let result = std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| {
            (mod_state.dispatch_fn)(event)
        })
    );

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            // Mod returned an error (not a panic)
            Err(ModError::DispatchFailed(e))
        }
        Err(panic_payload) => {
            // Mod panicked — quarantine it
            mod_state.quarantine(panic_payload);
            // Record error to channel
            ERROR_CHANNEL.push(PanicRecord {
                mod_handle,
                timestamp: Instant::now(),
                payload: format_panic(panic_payload),
            });
            // Return error but DON'T crash runtime
            Err(ModError::Panicked)
        }
    }
}
```

### 7.3 绝不跨 FFI unwind

```rust
// ✅ 正确：所有 extern "C" 函数内部都有 catch_unwind
#[unsafe(no_mangle)]
pub extern "C" fn morrow_tick(runtime_handle: u64) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // real tick logic
    }));
    // 即使内部 panic，也不会 unwind 到 C/JVM 栈帧
}

// ❌ 错误：裸的 extern "C" 没有 panic boundary
// 如果内部 panic 且 unwind 穿透 FFI → undefined behavior
```

---

## 八、Capability 系统（优化版）

### 8.1 设计理念

Capability 不是 trait object 也不是全局单例。它是**显式能力注册表**：

```rust
// SDK 视角：
let event_bus = ctx.capability::<EventBus>()?;
// ↑ 返回 Option，因为未来某些 cap 可能不存在或未就绪
```

### 8.2 Capability 注册表

```rust
pub enum Capability {
    EventBus(Arc<EventBus>),
    Registry(Arc<Registry>),
    Scheduler(Arc<Scheduler>),       // v2
    Commands(Arc<CommandDispatcher>), // v2
    Permissions(Arc<PermissionSys>),  // v2
}

impl Capability {
    pub fn capability_id(&self) -> CapabilityId { ... }
    pub fn version(&self) -> u32 { ... }
}
```

### 8.3 协商协议

```
Mod 启动时：
  Mod: "我需要 EventBus v1"
  Runtime: "我有 EventBus v1 ✓"

  Mod: "我需要 Commands v1"
  Runtime: "我没有 Commands ✗"
  Mod: → 自行处理: feature-gate command 功能
```

---

## 九、构建系统

### 9.1 开发流程

```
morrow/
├── runtime-rs/        cargo build → target/release/libmorrow_runtime.so
├── sdk-rs/            cargo build (rlib, 被 mod 依赖)
├── bridge-java/       gradle build → fabric mod .jar
├── examples/
│   └── hello-morrow/  cargo build → .morrow package
```

### 9.2 完整构建命令

```bash
# 1. Build Rust runtime
cd runtime-rs && cargo build --release

# 2. Copy .so to Java project
cp target/release/libmorrow_runtime.so ../bridge-java/fabric-host/src/main/resources/natives/

# 3. Build Java bridge (Gradle + Loom)
cd ../bridge-java/fabric-host && ./gradlew build

# 4. Build example mod
cd ../../examples/hello-morrow && cargo build --release --target x86_64-unknown-linux-gnu

# 5. Package as .morrow
morrow-cli package ./hello-morrow
```

### 9.3 CI 矩阵

```yaml
# .github/workflows/ci.yml
strategy:
  matrix:
    os: [windows-latest, ubuntu-latest]
    rust: [stable]
    java: [21]
```

---

## 十、路线图（带验收标准）

### Milestone 0: Environment + First Panama Call

**目标：** 证明 Rust ↔ Java 通信可行

**交付物：**
- [ ] JDK 21 安装验证
- [ ] Monorepo 工程结构创建
- [ ] Rust cdylib with `add(a: i32, b: i32) -> i32`
- [ ] Java Panama 代码调用 `add(2, 3)`
- [ ] 输出 `5`

**验收标准：** 终端打印 `5`

---

### Milestone 1: Minimal Runtime

**目标：** Runtime 骨架能 init/shutdown

**交付物：**
- [ ] `morrow_init()` / `morrow_shutdown()` 导出
- [ ] Runtime state machine 实现
- [ ] Opaque handle 分配/释放
- [ ] 从 Java 加载 .so 并调用 init/shutdown
- [ ] 验证：可重复 init → shutdown 10 次不泄漏

**验收标准：** 无 memory leak，handle 数量归零

---

### Milestone 2: Fabric Integration

**目标：** 在 Minecraft 里启动 Morrow

**交付物：**
- [ ] Fabric mod 骨架（MorrowHostMod）
- [ ] Native library 自动发现与加载
- [ ] Panama Bridge 初始化
- [ ] 生命周期 hook：JVM start → morrow_init
- [ ] 游戏启动时 log 输出 "Morrow initialized"

**验收标准：** Minecraft 启动，日志中看到 `[Morrow] Runtime initialized`

---

### Milestone 3: Rust Mod Loading

**目标：** 加载第一个 Rust 写的 Minecraft Mod

**交付物：**
- [ ] `.morrow` 包格式实现（ZIP + manifest 解析）
- [ ] 平台 artifact 选择逻辑
- [ ] `morrow_load_mod()` 完整实现
- [ ] Mod registry
- [ ] 示例 mod：输出 "Hello from Rust!" 到 Minecraft log

**验收标准：** Minecraft 日志中出现 Rust mod 的输出

---

### Milestone 4: Event Dispatch

**目标：** Rust mod 能响应游戏事件

**交付物：**
- [ ] Event bus 实现
- [ ] Fabric 事件 → Panama upcall → Rust dispatch
- [ ] Tick 事件转发
- [ ] Rust mod 响应 on_tick
- [ ] 示例：Rust mod 每 20 tick 打印一次玩家数

**验收标准：** Rust mod 成功接收并响应游戏 tick 事件

---

### Milestone 5: SDK Macros

**目标：** 提升开发者体验

**交付物：**
- [ ] `#[morrow::mod_main]` proc macro
- [ ] `Context` API 稳定
- [ ] Event listener derive macro
- [ ] 文档 + 示例

**验收标准：** 用 5 行 Rust 代码写出一个可加载的 mod

---

### Milestone 6: Linux Verification

**目标：** 验证 Linux 服务器部署

**交付物：**
- [ ] Linux native build
- [ ] Dedicated server 测试
- [ ] 修复平台差异问题
- [ ] CI: Ubuntu build + test

**验收标准：** Linux dedicated server 稳定运行 1 小时

---

### Milestone 7: Benchmark Suite

**目标：** 量化性能优势

**交付物：**
- [ ] Panama FFM vs JNI 对比 benchmark
- [ ] Tick overhead 测量（Rust mod vs 空 Fabric mod）
- [ ] Memory footprint 测量
- [ ] Event dispatch latency 测量

**验收标准：** 性能数据支撑 "native performance" 声明

---

## 十一、v1 完成标准

- [ ] Windows + Linux 双平台
- [ ] JDK 21
- [ ] Fabric Loader
- [ ] Rust mod SDK（proc macros）
- [ ] 稳定 ABI（版本化）
- [ ] Panic 隔离（mod crash ≠ server crash）
- [ ] 文档（API docs + getting started）
- [ ] >= 3 个示例 mod
- [ ] CI/CD pipeline
- [ ] Morrow 1.0.0 发布

---

## 十二、v2 方向（仅作记录，不执行）

- JDK 25 adapter（Panama 进一步优化）
- Quilt Loader 支持
- NeoForge 支持（如果社区需求大）
- 插件系统（mod 之间通信）
- Server extension API
- 热重载（谨慎评估，Panama 层的限制）
- macOS (Apple Silicon)
- 自研 Loader（摆脱 Fabric 依赖）
- WebAssembly 沙箱（替代 native lib 作为安全层？）

---

## 十三、现在开始

当前任务：**Milestone 0 — Environment + First Panama Call**

```
1. 安装 Eclipse Temurin JDK 21
2. 验证 Rust stable
3. 创建 monorepo
4. 写 Rust add() cdylib
5. 写 Java Panama caller
6. 看到 "5" 输出
```

不思考 v2。不放第一块砖，项目就不是真的。
