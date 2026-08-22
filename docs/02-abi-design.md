# 02 — ABI 设计规范

## ABI 版本

```rust
/// Morrow ABI version — 每次不兼容变更递增主版本
/// 兼容变更（新增函数）递增次版本
pub const ABI_VERSION: u32 = 0x0001_0000; // 主版本 1，次版本 0

/// ABI 兼容性检查
/// - 同主版本 = 兼容
/// - 主版本不同 = 不兼容
/// - 次版本不同 = 兼容（新函数 Java 侧不调用即可）
pub fn is_abi_compatible(requested: u32, actual: u32) -> bool {
    (requested >> 16) == (actual >> 16)  // 主版本相同
}
```

## 导出函数定义

所有导出函数使用 `extern "C"` + `#[unsafe(no_mangle)]`，调用约定为平台默认 C ABI。

### Runtime Lifecycle

```c
/**
 * 初始化 Morrow Runtime
 *
 * @param abi_version   调用者请求的 ABI 版本
 * @return              runtime_handle (u64), 0 表示初始化失败（版本不兼容）
 *
 * 调用时机：服务器 loadWorld 完成时（MinecraftServerMixin）
 * 线程安全：应在主线程调用一次
 */
uint64_t morrow_init(uint32_t abi_version);

/**
 * 关闭 Morrow Runtime
 *
 * @param runtime_handle   morrow_init 返回的 handle
 * @return                 0 成功，非零错误码
 *
 * 调用时机：服务器关闭前
 * 副作用：卸载所有已加载 mod，释放所有资源
 */
uint32_t morrow_shutdown(uint64_t runtime_handle);
```

### Mod Management

```c
/**
 * 加载一个 .mor 包
 *
 * @param runtime_handle
 * @param path_ptr         包文件系统路径（UTF-8, pointer+length）
 * @param path_len         路径长度
 * @return                 0 成功，非零错误码（详见 error channel）
 *
 * 行为（三段式，见 design.md §1.4）：
 * A. 锁内解析 manifest、校验依赖
 * B. 无锁提取 + dlopen + 调 mod entry point（mod 可重入 API）
 * C. 锁内注册 mod 与回调符号
 */
uint32_t morrow_load_mod(uint64_t runtime_handle,
                         uint64_t path_ptr,
                         uint32_t path_len);

// 无 morrow_unload_mod：卸载仅随 morrow_shutdown 整 runtime 释放
// （单独卸载涉及 dlclose 语义与跨 mod 引用，v2 再评估）
```

### Event Dispatch（批量派发，v0.12+ 生产路径）

```c
/**
 * 分发一整 tick 的事件批次（1 次 FFM 调用/tick）
 *
 * @param runtime_handle
 * @param data_ptr     off-heap 事件缓冲（Java EventBuffer 写入）
 * @param data_len     缓冲字节数
 *
 * 调用时机：每个 game tick 结束时（Mixin tick RETURN）
 * 缓冲格式见下方"事件类型码"规范表
 */
void morrow_dispatch_batch(uint64_t runtime_handle,
                           uint64_t data_ptr,
                           uint32_t data_len);
```

### 事件类型码（规范表 —— 单一事实源）

批量缓冲的 wire format：

```
u32le: total_events
for each:
  u16le: event_type   ← 见下表
  u16le: field1_len
  u16le: field2_len
  field1 bytes
  field2 bytes (may be empty)
```

tick 事件的 8 字节 tick 号直接放在 6 字节头之后（field1/2 为空）。

| Code | 事件 | field1 | field2 |
|------|------|--------|--------|
| 0 | tick | （空，头后跟 u64 tick 号） | （空） |
| 1 | player_join | 玩家名 | （空） |
| 2 | player_leave | 玩家名 | （空） |
| 3 | chat_message | 玩家名 | 消息 |
| 4 | block_break | 玩家名 | 方块 |
| 5 | block_place | 玩家名 | 方块 |
| 6 | player_death | 玩家名 | （空，cause 传 null） |

**同步义务**：此表是唯一权威。Java 侧写入（`bridge-java` EventBuffer）由
`EventBufferCodeTest` 钉住，Rust 侧解析由
`runtime-rs/tests/mod_loader_integration.rs` 钉住。改任何一侧都要改此表，
否则对应测试失败。

### Tick

```c
/**
 * 驱动 mod 的 on_tick 回调（legacy 单事件入口，测试用）
 *
 * @param runtime_handle
 * @param tick_number     tick 序号
 *
 * 调用时机：每个 game tick（20 TPS）
 * 生产路径走 morrow_dispatch_batch（见上），此入口保留给 Java 桥接测试
 */
void morrow_tick(uint64_t runtime_handle, uint64_t tick_number);
```

### Error Channel

```c
/**
 * 获取最后一个错误
 *
 * @param runtime_handle
 * @return                 error_handle, 0 表示无错误
 */
uint64_t morrow_last_error(uint64_t runtime_handle);

/**
 * 获取错误消息
 *
 * @param error_handle     morrow_last_error 返回的 handle
 * @param runtime_handle
 * @param buffer           输出缓冲区（调用者分配）
 * @param buffer_cap       缓冲区容量
 * @return                 实际写入的字节数（不含 null terminator）
 */
uint32_t morrow_error_message(uint64_t error_handle,
                               uint64_t runtime_handle,
                               uint64_t buffer,
                               uint32_t buffer_cap);

// 无 morrow_error_free / morrow_handle_free：error 记录被读取即消费
// （take 语义），handle 失效由 remove 触发释放
```

## 数据结构约定

### 字符串

所有字符串跨 FFI 传递采用：(pointer, length) 二元组。

```
  Java (caller)                    Rust (callee)
  ─────────────                    ──────────────
  分配 UTF-8 字节数组         →    读取字节数组
  (在 Arena 内)                    不分配，不释放
  Arena 关闭时自动释放              (借用语义)
```

**禁止：** 在 Rust 侧为 Java 传过来的字符串调用 `free()`。
**禁止：** 在 Java 侧为 Rust 返回的字符串调用 `free()`。

### 结构体

跨 FFI 的结构体使用显式内存布局：

```rust
/// 跨 FFI 传递的坐标结构
/// Layout: x(f64) | y(f64) | z(f64) = 24 bytes
#[repr(C)]
pub struct FVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// Java 侧对应：
// MemorySegment seg = arena.allocate(24);
// seg.set(ValueLayout.JAVA_DOUBLE, 0, x);
// seg.set(ValueLayout.JAVA_DOUBLE, 8, y);
// seg.set(ValueLayout.JAVA_DOUBLE, 16, z);
```

规则：
- 仅 `#[repr(C)]` struct 可跨 FFI 传递
- 所有字段类型必须是 FFI-safe（`i32`, `u64`, `f64`, `*const T` 等）
- 不允许：`String`, `Vec`, `Box<dyn Trait>`, `HashMap` 等非 FFI-safe 类型

### 所有权规则

**allocator 不跨边界**：谁分配谁释放。完整矩阵：

| 数据 | 分配方 | 释放方 | 机制 |
|------|--------|--------|------|
| Runtime state | Rust | Rust (`morrow_shutdown`) | Rust ownership |
| Mod 实例（dlopen 的库） | Rust | Rust (`morrow_shutdown`) | Rust ownership |
| 事件批量数据 | Java EventBuffer | Java (`reset()` close arena) | per-tick `Arena.ofConfined()` |
| 字符串 (Java → Rust) | Java | Java（arena close 或 GC） | Rust 只读借用，调用期间有效 |
| 字符串 (Rust → Java，upcall) | Rust | Rust | Java 在 upcall 返回前读完（`toArray`），不得留存指针 |
| 缓冲区 (Rust 写回 Java) | Java 调用者 | Java | Rust 只写、只读长度上限，绝不扩容 |
| Opaque handles | Rust | Rust | Arc 注册表，remove 即失效，悬空返回错误非 UB |

## ABI 稳定性保证

以下内容 **保证** 在 ABI 主版本内不变：

1. 函数签名（名称、参数类型、返回类型）
2. Opaque handle 的 u64 表示
3. 错误码定义
4. `#[repr(C)]` struct 的字段布局

以下内容 **不保证** 稳定：

1. Handle 内部的 Rust 对象布局
2. Arena 的内部实现
3. 错误消息的文本格式
4. 日志格式
