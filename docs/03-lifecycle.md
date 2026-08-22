# 03 — Runtime 生命周期（v0.16）

## 状态机（3 态）

```
              ┌─────────────────────────┐
              │      （Runtime 不存在）    │
              └────────────┬────────────┘
                           │ morrow_init(abi_version)
                           │ ABI 校验 + 建 Kernel + 入全局表
                           ▼
              ┌─────────────────────────┐
              │          Ready           │◄──────────┐
              │  mod 加载 / tick 派发 /   │           │
              │  命令 / Host API 全可用    │  begin_shutdown 失败
              └────────────┬────────────┘  （非法迁移）
                           │ morrow_shutdown()
                           ▼
              ┌─────────────────────────┐
              │      ShuttingDown        │
              │  卸 mod（dlclose）        │
              └────────────┬────────────┘
                           │ finish_shutdown()
                           ▼
              ┌─────────────────────────┐
              │          Dead            │
              │  Kernel 已从全局表移除并   │
              │  drop（所有注册表随之释放） │
              └─────────────────────────┘
```

状态转换在 [runtime-rs/src/runtime/state.rs](../runtime-rs/src/runtime/state.rs)，
非法迁移（重复 shutdown 等）返回错误码，不 panic。

## Mod 生命周期 Hooks（导出符号，非 trait）

Mod 通过 cdylib 导出符号被发现（design.md §1.5），SDK 宏生成：

| 导出符号 | 触发时机 | 签名 |
|---------|---------|------|
| `morrow_mod_init` | 包加载（`#[morrow::mod_main]`） | `fn(&mut Context, *const RuntimeApi) -> Result<(), MorrowError>` |
| `morrow_mod_tick` | 每 tick | `fn(u64)` |
| `morrow_mod_server_start` | 服务器启动完成 | `fn()` |
| `morrow_mod_server_stop` | 服务器停机前 | `fn()` |
| `morrow_mod_player_join` / `_leave` | 玩家进出 | `fn(&str)` |
| `morrow_mod_chat_message` | 聊天 | `fn(&str, &str)` |
| `morrow_mod_block_break` / `_place` | 方块破坏/放置 | `fn(&str, &str)` |
| `morrow_mod_player_death` | 玩家死亡 | `fn(&str, &str)` |

加载时由 runtime 扫描符号并注册进对应注册表（lock 内，见下）。

## 加载时序（MorrowMod.init，Mixin 在 loadWorld RETURN 触发）

```
T=0      服务器 loadWorld 完成 → MinecraftServerMixin.onLoadWorld → MorrowMod.init
           │
           ├─ 1. NativeLibraryLoader.load()   → libmorrow_runtime.so
           ├─ 2. PanamaBridge.create()        → SymbolLookup + downcall handles
           ├─ 3. morrow_init(ABI_VERSION)     → runtime_handle（版本不符 = 0，拒绝启动）
           ├─ 4. 扫描 mods/*.mor → 逐个 morrow_load_mod（三段式，design.md §1.4）
           │     失败包重试一轮（依赖排序兜底）
           ├─ 5. 绑定 morrow_dispatch_batch downcall
           ├─ 6. morrow_dispatch_server_start
           └─ 7. 注册 HostVtable（7 个 upcall stub）→ mod API 就绪
```

## 每 tick 数据流

```
tick HEAD    EventBuffer.tick(n)            ← 只写 buffer，不碰 FFI
tick 期间    join/chat/block 等事件追加进 buffer
tick RETURN  flushBatch() → morrow_dispatch_batch（1 次 downcall）
               Rust：锁内快照回调表 → 锁外逐个 dispatch（各自 catch_unwind）
               → panic 的 mod 进 Quarantine，后续 tick 跳过
最后         eventBuffer.reset() → close per-tick arena
```

## 错误恢复策略

| 错误场景 | 行为 |
|---------|------|
| Mod init 返回 Err / panic | Mod 不加载，记录日志，不影响其他 mod |
| Mod 回调 panic（tick/事件/命令） | catch_unwind 隔离，mod 进 Quarantine，服务器继续 |
| Runtime 入口 panic | `ffi_boundary` 兜底——绝不 unwind 穿 FFI 边界 |
| Native crash (SIGSEGV) | 进程级故障，无恢复（Layer 1/2 的设计目标就是永不走到这层） |
| 重复 shutdown / 非法状态迁移 | 返回错误码，不 panic |
