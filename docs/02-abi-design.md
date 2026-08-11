# 02 — ABI 设计规范

## ABI 版本

```rust
/// Ferrum ABI version — 每次不兼容变更递增主版本
/// 兼容变更（新增函数）递增次版本
pub const ABI_VERSION: u32 = 1;

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
 * 初始化 Ferrum Runtime
 *
 * @param abi_version   调用者请求的 ABI 版本
 * @param config_ptr    指向配置 JSON 的指针（UTF-8, null-terminated）
 * @param config_len    配置数据长度（字节）
 * @return              runtime_handle (u64), 0 表示初始化失败
 *
 * 调用时机：JVM 启动后，Fabric 初始化完成时
 * 线程安全：应在主线程调用一次
 * 错误处理：失败返回 0，错误信息通过 ferrum_last_error 获取
 */
uint64_t ferrum_init(uint32_t abi_version,
                     uint64_t config_ptr,
                     uint32_t config_len);

/**
 * 关闭 Ferrum Runtime
 *
 * @param runtime_handle   ferrum_init 返回的 handle
 * @return                 0 成功，非零错误码
 *
 * 调用时机：服务器关闭前
 * 副作用：卸载所有已加载 mod，释放所有资源
 */
uint32_t ferrum_shutdown(uint64_t runtime_handle);
```

### Mod Management

```c
/**
 * 加载一个 .ferrum 包
 *
 * @param runtime_handle
 * @param package_path     .ferrum 文件的文件系统路径（UTF-8）
 * @param path_len          路径长度
 * @return                 mod_handle (u64), 0 表示加载失败
 *
 * 行为：
 * 1. 解析 manifest.toml
 * 2. 验证 ABI 兼容性
 * 3. 选择平台 artifact
 * 4. 加载动态库
 * 5. 调用 mod entry point
 * 6. 注册到 mod registry
 */
uint64_t ferrum_load_mod(uint64_t runtime_handle,
                          uint64_t package_path,
                          uint32_t path_len);

/**
 * 卸载一个 mod
 *
 * @param runtime_handle
 * @param mod_handle       ferrum_load_mod 返回的 handle
 * @return                 0 成功
 */
uint32_t ferrum_unload_mod(uint64_t runtime_handle,
                            uint64_t mod_handle);
```

### Event Dispatch

```c
/**
 * 分发事件到所有注册的 mod
 *
 * @param runtime_handle
 * @param event_type      事件类型名称（UTF-8, 如 "server.tick"）
 * @param type_len        类型名长度
 * @param event_data      事件数据（JSON 或二进制）
 * @param data_len        数据长度
 * @return                处理该事件的 mod 数量
 *
 * 线程安全：可从 Java 事件线程调用
 */
uint32_t ferrum_dispatch_event(uint64_t runtime_handle,
                                uint64_t event_type,
                                uint32_t type_len,
                                uint64_t event_data,
                                uint32_t data_len);

/**
 * 注册 Java 端事件回调（用于 Rust → Java 通信）
 *
 * @param runtime_handle
 * @param event_type      事件类型名称
 * @param type_len
 * @param callback_addr    upcall stub 的内存地址
 * @return                 0 成功
 */
uint32_t ferrum_register_upcall(uint64_t runtime_handle,
                                 uint64_t event_type,
                                 uint32_t type_len,
                                 uint64_t callback_addr);
```

### Tick

```c
/**
 * 驱动 mod 的 on_tick 回调
 *
 * @param runtime_handle
 *
 * 调用时机：每个 game tick（20 TPS）
 * 性能要求：必须在 <1ms 内返回
 */
void ferrum_tick(uint64_t runtime_handle);
```

### Error Channel

```c
/**
 * 获取最后一个错误
 *
 * @param runtime_handle
 * @return                 error_handle, 0 表示无错误
 */
uint64_t ferrum_last_error(uint64_t runtime_handle);

/**
 * 获取错误消息
 *
 * @param error_handle     ferrum_last_error 返回的 handle
 * @param buffer           输出缓冲区（调用者分配）
 * @param buffer_cap       缓冲区容量
 * @return                 实际写入的字节数（不含 null terminator）
 */
uint32_t ferrum_error_message(uint64_t error_handle,
                               uint64_t buffer,
                               uint32_t buffer_cap);

/**
 * 释放 error handle
 *
 * @param error_handle
 */
void ferrum_error_free(uint64_t error_handle);
```

### 资源管理

```c
/**
 * 释放 handle（通用的 handle 析构）
 *
 * @param handle   任何由 Ferrum 分配的 handle
 *
 * 注意：runtime_handle 不通过此函数释放（使用 ferrum_shutdown）
 */
void ferrum_handle_free(uint64_t handle);
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
