# 01 — 完整架构

## 架构全景图

```
                    Minecraft (Java) — 游戏主进程
                          │
          ┌───────────────┼───────────────┐
          │               │               │
     Fabric API      Java Mod A      Java Mod B
          │
     FerrumHostMod (Fabric ModInitializer)
          │
┌─────────┼─────────┐
│   Panama FFM API   │  ← JDK 21 java.lang.foreign
│   ┌─────────────┐  │
│   │  Linker      │  │  System linker (libc calling convention)
│   │  Arena       │  │  Confined arena per tick
│   │  MemorySeg   │  │  Off-heap zero-copy buffers
│   └──────┬───────┘  │
│          │          │
│   Downcall Handles  │  Java → Rust calls
│   Upcall Stubs      │  Rust → Java callbacks
└──────────┼──────────┘
           │ FFI Boundary (C ABI)
┌──────────┼──────────┐
│   extern "C" fns    │  Stable ABI entry points
│   ┌──────┴───────┐  │
│   │ Panic Layer   │  │  catch_unwind at every FFI entry
│   │ ┌───────────┐ │  │
│   │ │  Kernel    │ │  │  mod registry + event bus + scheduler
│   │ │ ┌───────┐ │ │  │
│   │ │ │ Mod A │ │ │  │  dlopen'd .so (or static linked?)
│   │ │ │ Mod B │ │ │  │
│   │ │ │ Mod C │ │ │  │
│   │ │ └───────┘ │ │  │
│   │ └───────────┘ │  │
│   └───────────────┘  │
│  Ferrum Runtime Core │  Rust cdylib
└──────────────────────┘
```

## 分层详解

### Layer 0: Fabric Loader + API

Fabric Loader 负责：
- 类加载（Mixin、mod JAR 发现）
- 生命周期调度
- Mixin 注入

Ferrum 不修改 Fabric Loader，只作为一个普通的 Fabric Mod 存在。

**关键依赖：**
- `fabric-loader`: 加载 FerrumHostMod
- `fabric-api`: 事件系统、注册表等

### Layer 1: Ferrum Host Adapter (Java, runs inside JVM)

**模块分解：**

```
com.ferrum.host
├── FerrumMod.java          # implements ModInitializer
├── NativeLibraryLoader.java # 平台感知的 .so/.dll 加载
├── PanamaBridge.java        # 一次性初始化 Panama 链接
├── LifecycleCoordinator.java # 管理 JVM ↔ Rust 生命周期同步
├── EventDispatcher.java     # Fabric events → Rust dispatch
├── CapabilityChannel.java   # Capability 协商
├── ArenaManager.java        # Arena 分配策略
└── NativeCrashDetector.java  # 检测 native 崩溃并尝试恢复
```

**FerrumMod.java 启动序列：**

```java
public class FerrumMod implements ModInitializer {
    @Override
    public void onInitialize() {
        // 1. Platform detection
        Platform platform = Platform.detect();  // os.name, os.arch

        // 2. Load native runtime
        Path runtimeLib = NativeLibraryLoader.load(
            platform, "ferrum_runtime");

        // 3. Setup Panama bridge
        PanamaBridge bridge = PanamaBridge.create(runtimeLib);

        // 4. Initialize runtime
        long runtimeHandle = bridge.ferrumInit(ABI_VERSION);

        // 5. Discover and load mods
        List<Path> modPackages = ModDiscovery.scan("mods/");
        for (Path pkg : modPackages) {
            bridge.ferrumLoadMod(runtimeHandle, pkg);
        }

        // 6. Attach lifecycle hooks
        LifecycleCoordinator.attach(runtimeHandle, bridge);

        // 7. Start event dispatch
        EventDispatcher.start(runtimeHandle, bridge);
    }
}
```

### Layer 2: Panama Bridge

**为什么 Panama 优于 JNI：**

JNI 有三个致命问题：

1. **GlobalRef/LocalRef 管理复杂** — Java GC 与 native 生命周期耦合，遗忘 DeleteLocalRef 导致内存泄漏
2. **类型系统薄弱** — `jint`, `jlong`, `jobject` 不够表达复杂类型
3. **调用开销** — JNI 调用路径经过多层 JVM 内部转换

Panama FFM 直接解决了这三个问题：

- **Arena** — 显式内存作用域，arena.close() 自动释放所有分配，杜绝泄漏
- **ValueLayout** — 类型安全的内存布局描述，编译期可检查
- **MethodHandle** — JIT 可内联 downcall，理论延迟低至 0 cycles

**Downcall 示例（Java → Rust）：**

```java
// 一次性初始化（启动时）
Linker linker = Linker.nativeLinker();
SymbolLookup runtime = SymbolLookup.libraryLookup(
    Path.of("libferrum_runtime.so"), Arena.global());

MethodHandle ferrum_init = linker.downcallHandle(
    runtime.find("ferrum_init").orElseThrow(),
    FunctionDescriptor.of(
        ValueLayout.JAVA_LONG,  // return: Handle
        ValueLayout.JAVA_INT    // param: abi_version
    ));

// 调用（每次）
long handle = (long) ferrum_init.invokeExact(1);
```

**Upcall 示例（Rust → Java 回调）：**

```java
// Java side: 创建 upcall stub 传给 Rust
MethodHandle onEvent = linker.upcallStub(
    MethodHandles.lookup().findStatic(
        EventDispatcher.class, "onRustEvent",
        MethodType.methodType(void.class, long.class, long.class)
    ),
    FunctionDescriptor.ofVoid(
        ValueLayout.JAVA_LONG,  // event_type_ptr
        ValueLayout.JAVA_LONG   // event_data_ptr
    ),
    Arena.global()
);

// 把 upcall stub 的内存地址传给 Rust
ferrum_register_callback(runtimeHandle,
    /* event_type */ "server.tick",
    /* callback */ onEventStub.address()
);
```

### Layer 3: Ferrum Runtime Core (Rust)

**内部架构：**

```
runtime-rs/
├── src/
│   ├── lib.rs          # extern "C" exports + module tree
│   ├── abi/            # ABI contract implementation
│   │   ├── mod.rs
│   │   ├── handles.rs   # Opaque Handle<T> implementation
│   │   └── arena.rs     # Rust-side Arena (wraps Java Arena ptr)
│   ├── kernel/
│   │   ├── mod.rs       # RuntimeKernel struct + state machine
│   │   ├── registry.rs  # ModRegistry: Map<ModId, LoadedMod>
│   │   └── config.rs    # Runtime configuration
│   ├── event/
│   │   ├── mod.rs       # EventBus: Vec<EventListener>
│   │   ├── types.rs     # Standard event type definitions
│   │   └── dispatch.rs  # dispatch_event implementation
│   ├── mod_loader/
│   │   ├── mod.rs       # Mod loading orchestration
│   │   ├── manifest.rs  # .ferrum manifest parsing
│   │   └── artifact.rs  # Platform artifact selection
│   ├── cap/
│   │   ├── mod.rs       # CapabilityRegistry
│   │   └── types.rs     # CapabilityId, Capability trait
│   ├── panic.rs         # Panic boundary utilities
│   ├── error.rs         # ErrorChannel implementation
│   └── util/
│       ├── strings.rs   # FFI string helpers
│       └── ffi.rs       # Common FFI utilities
```

**Handle 系统的实现：**

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Opaque handle — 对外是 u64，内部映射到具体对象
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(u64);

impl Handle {
    pub fn new() -> Self {
        Handle(NEXT_HANDLE.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u64(self) -> u64 { self.0 }
    pub fn from_u64(raw: u64) -> Self { Handle(raw) }
}

/// 带 generation 的 handle（防止 use-after-free）
#[derive(Debug)]
struct HandleEntry<T> {
    generation: u32,
    data: Option<T>,
}

pub struct HandleTable<T> {
    entries: Vec<HandleEntry<T>>,
    free_list: Vec<usize>,
}
```

### Layer 4: SDK (Rust, for mod developers)

SDK 是对 Runtime Core ABI 的 ergonomic 封装。

**开发者视角：**

```rust
use ferrum::prelude::*;

#[ferrum::mod_main]
fn init(ctx: &mut Context) -> Result<(), FerrumError> {
    // 注册事件监听
    ctx.event_bus()?.on::<ServerTick>(|event| {
        if event.tick_number % 20 == 0 {
            // 获取在线玩家
            let players = event.server().online_players();
            ferrum::info!("Online players: {}", players.len());
        }
    });

    // 注册命令
    ctx.commands()?.register("hello", |args| {
        ferrum::info!("Hello, {}!", args.get(0).unwrap_or(&"world".into()));
    });

    Ok(())
}
```

### 数据流

**Tick 数据流（最热路径）：**

```
Minecraft Server Thread
  │
  ├─ Tick N begins
  │
  ├─ Fabric ServerTickCallback (Java)
  │   └─ EventDispatcher.onServerTick()
  │       └─ try (Arena arena = Arena.ofConfined()) {
  │             // 1. 写入 tick 数据到 off-heap MemorySegment
  │             MemorySegment tickData = arena.allocate(8);
  │             tickData.set(JAVA_LONG, 0, tickNumber);
  │
  │             // 2. 调用 Rust (单次 downcall)
  │             ferrum_tick.invokeExact(runtimeHandle);
  │
  │             // 3. Rust 内部通过 upcall 获取 tick data
  │             //    或直接读 MemorySegment
  │
  │             // 4. arena 自动释放 tickData
  │           }
  │
  ├─ Fabric tick processing continues (Java mods)
  │
  └─ Tick N ends
```

**延迟预算：**
- Panama downcall: ~10ns
- Arena allocate: ~20ns
- Rust tick dispatch: depends on mod, target <100μs for empty mod
- 总量目标: <1ms per tick for reasonable mod count
