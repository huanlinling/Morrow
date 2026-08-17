# Morrow 设计决策记录

> 只记**决策与理由**（为什么），不重复现状（是什么）。
> 现状：[docs/01-architecture.md](docs/01-architecture.md) ｜ ABI 契约：[docs/02-abi-design.md](docs/02-abi-design.md) ｜ 路线图：[docs/07-roadmap.md](docs/07-roadmap.md)
>
> 版本: 2.0 | 日期: 2026-08-17 | 状态: 现行

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
- Java Host 桥接（Java Agent + Mixin → Panama FFI，v0.12 起独立于 Fabric）
- 跨平台 Native Artifact 加载（`.morrow` 包格式）
- 稳定 ABI（opaque handles, no struct exposure）
- Runtime 生命周期管理
- Host Capability 抽象（registry, events, 未来扩展）
- Panic 隔离与错误边界

### 性能定位（v0.16 评审结论，2026-08-17）

**三层判断：**

1. **桥接机制**：Panama FFM 是 JVM 上最快的 native 调用路径（实测 9.3-9.7ns/call）。
2. **派发架构（真正的性能决策）**：批量派发把 FFI 边界穿越压成**每 tick 1 次**——
   开销与 mod 数量、事件数量无关。mod 扇出在 native 侧以 fn 指针完成（~1ns），
   分配收敛为每 tick 一个 confined arena（无 Java 堆往返、无 GC 压力）。
   对比逐事件 FFI（O(事件×mod) 次穿越 + 每事件分配），批量把穿越**次数**优化
   ~1000 倍，而 Panama vs JNI 只是把**单价**优化 ~3 倍——次数是杠杆，单价不是。
3. **性能已到终点**：空 tick 派发实测 0.04μs，占 Minecraft 50ms tick 预算的
   0.00008%。loader 开销不是瓶颈，mod 代码才是。桥接层不再投入优化——M7
   基准的性质是**验收**（向外部证明承诺），不是**研究**（找瓶颈）。

**批量换来的代价（对我们免费）**：事件最多延迟一个 tick（50ms）送达——
Minecraft 本身就是 20 TPS 快照模型，此延迟无人感知；若未来做实时型事件
系统（非 MC 场景），这是需要重新权衡的点。

---

## 一、核心决策与理由

### 1.1 Panama FFM 而非 JNI

| 维度 | JNI | Panama FFM |
|------|-----|------------|
| 调用开销 | ~20-30ns | ~5-10ns（inline 可达 0） |
| 内存管理 | 手动 GlobalRef/LocalRef 管理 | Arena 作用域管理，自动释放 |
| 类型安全 | JNI 类型系统老旧 | ValueLayout 类型安全 |
| 代码量 | Java + C glue code | 纯 Java，直接 downcall |
| 运行时 | 需要加载 JNI 库 | JDK 内置，无需额外 Runtime |
| 未来 | 遗留 API | JDK 官方主推方向 |

**结论：Panama 是现代 JDK 的 native 互操作标准答案。Morrow 全栈使用 Panama FFM API。**

### 1.2 批量派发：1 次 FFI/tick

决策：Java 侧把整 tick 事件累积进 off-heap `EventBuffer`，tick 结束时一次
`morrow_dispatch_batch` 交给 Rust，Rust 在 native 侧扇出到各 mod。

理由（数量级见 §零）：
- **穿越次数 O(1)**：朴素逐事件 FFI 是 O(事件数)，更差的逐事件×逐 mod
  派发是 O(事件×mod)。批量把次数这个杠杆压到底。
- **分配收敛**：每 tick 一个 confined arena，避免逐事件分配的 GC 压力
  （GC 停顿比 FFI 延迟贵得多）。
- **扇出位置**：Java 侧不需要知道 mod 列表——注册表和分发都在 Rust，
  跨语言边界只渡数据，不渡控制流。
- **事务边界**：tick N 的事件在 tick N+1 前一次性有序送达，mod 看到的
  世界状态一致（与 WorldSnapshot 每 tick 刷一次对齐）。

代价：事件延迟 ≤50ms 送达（见 §零，MC 场景免费）。

### 1.3 单锁内核 + 锁外调用铁律

决策：所有 per-runtime 状态放一把 `Mutex<RuntimeData>`（v0.16 前是 8 个
全局 map）；任何回调都**在锁内收集（clone fn 指针）、在锁外调用**。

理由：tick 派发只需抢一次锁；mod 回调重入任意 Runtime API 都不死锁
（v0.16 集成测试真实抓过 commands 跨锁重入死锁）。逃逸口：若未来争抢
可测（M7 会看），拆注册表级锁是机械改动，不是架构重写。

### 1.4 三段式 mod 加载

决策：`morrow_load_mod` 分三段——A 锁内解析 manifest/查依赖，B 无锁
dlopen + 调 init，C 锁内注册。

理由：mod init 代码可以重入 Runtime API（注册命令、读配置），任何
runtime 锁在 init 期间被持有都会死锁或阻塞其他调用方。纯文件 IO 段
也不该占锁。

### 1.5 符号发现（export symbol）而非注册式 EventBus

决策：事件回调通过 mod cdylib 导出符号（`morrow_mod_tick` 等，
`#[morrow::event(kind)]` 宏生成）被发现，而非 mod 主动调注册 API。

理由：与 ABI 契约（符号名即接口）一致，加载器不必在 mod 代码运行前
建立事件系统；注册式 EventBus（优先级、Arc 分发）是复杂度，推迟到 v2
（docs/07）。

### 1.6 Handle：u64 而非指针

理由：指针暴露内存布局、依赖架构位宽，跨版本即 ABI 崩坏。u64 是
架构无关的不透明值，0 恒为"无效"；有效性由 Arc 注册表保证——remove
即失效，悬空 handle 查表失败返回错误而非 UB。

---

## 二、ABI 核心原则

（函数签名与 wire format 见 docs/02，这里只有原则。）

1. **不暴露 Rust struct layout** — 跨 FFI 数据一律 opaque handle (u64) 或
   `#[repr(C)]` 显式布局。
2. **不跨 FFI unwind** — Rust panic 绝不穿透 FFI 边界，每个 extern "C"
   入口包 `catch_unwind`。
3. **allocator 不跨边界** — Rust 分配的由 Rust 释放，Java 分配的由 Java
   释放（完整所有权表见 docs/02 §所有权规则）。
4. **字符串用 (pointer, length)** — 借用语义，Rust 侧零拷贝读取，
   两侧都不得 free 对方传过来的字符串。
5. **版本协商前置** — `morrow_init(abi_version)` 时校验，主版本不兼容
   直接拒绝；新增 API 走 capability 协商而不是改 ABI。

---

## 三、Panic 隔离分层

```
Layer 1: Mod 回调 panic
  ├─ 每个回调独立 catch_unwind（tick/事件/命令）
  ├─ panic 的 mod 进 Quarantine，后续 tick 跳过，其他 mod 继续
  └─ 服务器无感

Layer 2: Runtime 入口 panic
  ├─ 每个 extern "C" 导出包 ffi_boundary（catch_unwind 兜底）
  └─ 绝不 unwind 到 C/JVM 栈帧（那是 UB）

Layer 3: 不可恢复崩溃（SIGSEGV / 栈溢出）
  └─ 进程级：native 崩溃 = JVM 崩溃，无恢复可能——靠 Layer 1/2 保证
     永远到不了这一层
```

Upcall 方向同理：Rust 调 Java 宿主函数的每一处都包 `catch_unwind`，
Java 侧异常/崩溃不会反向穿透。

---

## 四、Capability 设计

**为什么是显式注册表而不是 trait object / 全局单例：**
- trait object 跨 FFI 不成立（ABI 稳定性第一条）；全局单例违背"内核
  无全局状态"原则。
- 协商协议：mod 请求 `"capability_name"` → runtime 返回版本号或 0
  （不存在）。mod 自行 feature-gate，运行时可按需增长而不破坏旧 mod。
- 内置 capability（v0.16）：event_bus / commands / host_api / config /
  lifecycle / player_events / block_events / panic_isolation，各 v1。
- SDK 侧类型化访问器（`ctx.capability::<T>()`）留到有真实需求时再加。

---

## 五、明确排除（首版不做）

- ❌ Java 8 / 17（Panama 不可用或预览）
- ❌ Forge / NeoForge（Mixin 注入点理论可移植，社区有需求再谈）
- ❌ Fabric API 依赖（v0.12 起只把 Fabric 当类加载器，不碰其 API）
- ❌ 热重载（dlopen 卸载语义 + Panama 层限制，v2 谨慎探索）
- ❌ macOS（Apple Silicon 下 Panama + native lib 坑多）
- ❌ 插件系统 / mod 间通信协议（先稳住核心，别过早抽象）
- ❌ WASM sandbox（替代 native lib 的安全方案，极长期）

---

## 六、演进备忘

- v2 方向清单与状态在 docs/07-roadmap.md（单一事实源）。
- 架构现状改动后必须同步 docs/01；ABI 改动必须同步 docs/02 并
  递增版本；本文件只记"为什么"，不再描述"是什么"。
