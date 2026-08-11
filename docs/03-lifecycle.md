# 03 — Runtime 生命周期

## 状态机

```
              ┌─────────────────────────┐
              │       UNINITIALIZED      │
              │   Runtime 不存在          │
              └────────────┬────────────┘
                           │ ferrum_init()
                           │ (ABI version check)
                           │ (Memory allocation)
                           ▼
              ┌─────────────────────────┐
              │        INITIALIZING      │
              │   Runtime 正在初始化       │
              │   - Capability 注册表初始化 │
              │   - Event bus 初始化       │
              │   - 内部结构分配            │
              └────────────┬────────────┘
                           │ success
                           ▼
              ┌─────────────────────────┐
              │          READY           │◄──────────────┐
              │   Runtime 就绪，等待 Mod   │               │
              │   load                    │               │
              └────────────┬────────────┘               │
                           │ ferrum_load_mod()           │
                           │ (可调用多次)                  │
                           ▼                             │
              ┌─────────────────────────┐               │
              │      LOADING_MOD        │               │
              │   解析 manifest          │               │
              │   加载 .so/.dll          │               │
              │   调用 mod init          │               │
              └───────┬─────────┬───────┘               │
                      │ success │ failure                │
                      ▼         ▼                        │
              ┌──────────┐ ┌──────────────┐             │
              │  READY   │ │ READY        │             │
              │ (mod in  │ │ (error       │             │
              │ registry)│ │  recorded)   │             │
              └────┬─────┘ └──────┬───────┘             │
                   │              │                      │
                   └──────┬───────┘                      │
                          │ server starts                 │
                          ▼                              │
              ┌─────────────────────────┐               │
              │        RUNNING           │               │
              │   - 事件分发              │               │
              │   - tick dispatch        │               │
              │   - mod 正常运转          │               │
              └────────────┬────────────┘               │
                           │                             │
                           ▼                             │
              ┌─────────────────────────┐               │
              │        TICKING           │               │
              │   ferrum_tick() 调用中    │───────────────┘
              │   - 分发 on_tick 事件     │   tick 结束
              │   - arena 分配临时内存     │   回到 RUNNING
              │   - catch_unwind 隔离    │
              └───────┬─────────┬───────┘
                      │ success │ mod panic
                      ▼         ▼
              ┌──────────┐ ┌──────────────────┐
              │ RUNNING  │ │ RUNNING           │
              │ (正常)    │ │ (mod X quarantined)│
              └──────────┘ └──────────────────┘
                           │
                      server stops / crash
                           │
                           ▼
              ┌─────────────────────────┐
              │      SHUTTING_DOWN       │
              │   - mod on_shutdown 钩子  │
              │   - 释放所有 mod 资源     │
              │   - 关闭 event bus       │
              └────────────┬────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │          DEAD            │
              │   Runtime 已销毁          │
              └─────────────────────────┘
```

## Mod 生命周期 Hooks

```rust
/// Mod 必须实现的 trait
pub trait FerrumMod: Send + Sync + 'static {
    /// 元数据
    fn metadata(&self) -> ModMetadata;

    /// ── Lifecycle Hooks ──

    /// Mod 被加载并初始化
    /// 此时可以注册事件监听器、获取 capabilities
    fn on_init(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
        Ok(())
    }

    /// 服务端启动完成，世界已加载
    fn on_server_start(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
        Ok(())
    }

    /// 每个游戏 tick
    /// ⚠️ 必须在 <1ms 内返回！不要在此做 IO 或复杂计算
    fn on_tick(&mut self, ctx: &mut Context, tick: u64) -> Result<(), FerrumError> {
        Ok(())
    }

    /// 服务端即将停止，保存数据
    fn on_server_stop(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
        Ok(())
    }

    /// Mod 被卸载
    fn on_shutdown(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
        Ok(())
    }
}
```

## 加载时序细节

```
T=0ms    JVM 启动
T=50ms   Fabric Loader 开始扫描 mods/
T=100ms  Fabric 发现 FerrumHostMod (Java jar)
T=150ms  FerrumMod.onInitialize() 被调用
           │
T=155ms    ├─ NativeLibraryLoader.load("ferrum_runtime")
           │   ├─ 检测平台: os.name="Linux", os.arch="amd64"
           │   ├─ 搜索路径:
           │   │   1. <jar>/natives/linux-x86_64/libferrum_runtime.so
           │   │   2. mods/ferrum/native/linux-x86_64/libferrum_runtime.so
           │   │   3. 系统库路径
           │   └─ System.load(path) → dlopen
           │
T=160ms    ├─ PanamaBridge.setup()
           │   ├─ SymbolLookup.libraryLookup(path, Arena.global())
           │   ├─ 查找所有需要的符号 (ferrum_init, ferrum_load_mod, ...)
           │   └─ 创建 downcall MethodHandles
           │
T=165ms    ├─ ferrum_init(ABI_VERSION=1)
           │   ├─ Rust: 检查 ABI 版本
           │   ├─ Rust: 初始化 RuntimeKernel
           │   ├─ Rust: 返回 runtime_handle
           │   └─ log: "[Ferrum] Runtime initialized (ABI v1)"
           │
T=170ms    ├─ ModDiscovery.scan("mods/")
           │   发现: mods/hello-ferrum.ferrum
           │
T=175ms    ├─ ferrum_load_mod(runtime_handle, "mods/hello-ferrum.ferrum")
           │   ├─ Rust: 读取 ZIP 中的 manifest.toml
           │   ├─ Rust: 验证 api_version 兼容
           │   ├─ Rust: 选择 artifact: linux-x86_64/libmod.so
           │   ├─ Rust: 提取到临时目录
           │   ├─ Rust: dlopen(libmod.so)
           │   ├─ Rust: 查找 ferrum_mod_init 符号
           │   ├─ Rust: 调用 ferrum_mod_init(ModSdkVtable)
           │   └─ Rust: 将 mod 注册到 Registry
           │   └─ log: "[Ferrum] Loaded mod: hello-ferrum v0.1.0"
           │
T=200ms    Fabric 初始化完成
           │
T=200ms+   Minecraft 启动完成，进入游戏循环
           │
           ├─ Tick 0: ferrum_tick(runtime_handle)
           │   └─ 分发 on_tick 到所有 mod
           ├─ Tick 1: ferrum_tick(runtime_handle)
           │   └─ 分发 on_tick
           ├─ ...
           │
           ▼  (server stop)
T=∞       ferrum_shutdown(runtime_handle)
           ├─ Rust: 遍历所有 mod，调用 on_shutdown
           ├─ Rust: 卸载所有动态库
           ├─ Rust: 释放所有 handle
           └─ Rust: RuntimeKernel 析构
```

## 错误恢复策略

| 错误场景 | 行为 |
|---------|------|
| Mod init 返回 Err | Mod 不加载，记录日志，不影响其他 mod |
| Mod init panic | catch_unwind，mod 不加载，记录日志 |
| Mod on_tick 返回 Err | 记录日志，继续下次 tick |
| Mod on_tick panic | catch_unwind，隔离该 mod，其他 mod 继续 |
| Runtime kernel panic | Runtime 进入 degraded 模式，记录错误 |
| Native crash (SIGSEGV) | 进程级故障，JVM 可能会捕获（视 OS） |

## 生命周期相关 API

```rust
// SDK 提供
pub trait ModLifecycleExt: FerrumMod {
    /// 注册为生命周期回调
    fn register(self, ctx: &mut Context) -> Result<(), FerrumError>;
}
```
