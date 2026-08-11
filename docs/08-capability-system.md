# 08 — Capability 系统设计

## 设计理念

Capability 系统解决一个核心问题：**Mod 需要 Runtime 提供的能力，但不是所有能力在所有时刻都可用。**

传统做法（全局变量 / 单例）的问题：
- 隐式依赖，难以测试
- 不表达 "不存在"，调用者不知道某个功能是否可用
- 难以演化 — 新增能力时所有 mod 都需要重新编译

Ferrum 的 Capability 系统：

```rust
// Mod 显式声明需要什么
let event_bus = ctx.capability::<EventBus>()?;
// ↑ 返回 Result，因为可能不存在（v2 引入，或者运行时未启用）
```

## Capability Trait

```rust
/// 所有 Capability 实现此 trait
pub trait Capability: Send + Sync + 'static {
    /// 唯一标识
    fn id() -> CapabilityId;

    /// Capability 版本
    fn version() -> u32;

    /// 人类可读名称
    fn name() -> &'static str;
}

/// Capability ID — 编译期确定，全局唯一
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityId(u64);

impl CapabilityId {
    pub const fn from_name(name: &str) -> Self {
        // 编译期哈希
        CapabilityId(compile_time_hash(name))
    }
}
```

## 内置 Capability 清单

| Capability | ID | v1 | 说明 |
|-----------|-----|----|------|
| `EventBus` | `0x01` | ✅ | 事件注册与分发 |
| `CommandRegistry` | `0x02` | ❌ v2 | 命令注册 |
| `Scheduler` | `0x03` | ❌ v2 | 定时任务 |
| `PermissionSystem` | `0x04` | ❌ v2 | 权限管理 |
| `NetworkChannel` | `0x05` | ❌ v2 | 网络通信 |
| `ConfigStore` | `0x06` | ✅ | 配置读写 |
| `PlayerAPI` | `0x07` | ❌ v2 | 玩家操作 |
| `WorldAPI` | `0x08` | ❌ v2 | 世界操作 |
| `ItemAPI` | `0x09` | ❌ v2 | 物品操作 |
| `BlockAPI` | `0x0A` | ❌ v2 | 方块操作 |

## 注册与发现

### Runtime 侧

```rust
pub struct CapabilityRegistry {
    capabilities: HashMap<CapabilityId, Box<dyn Any + Send + Sync>>,
    versions: HashMap<CapabilityId, u32>,
}

impl CapabilityRegistry {
    /// 注册一个 capability
    pub fn register<C: Capability>(&mut self, capability: C) {
        self.capabilities.insert(C::id(), Box::new(capability));
        self.versions.insert(C::id(), C::version());
    }

    /// 获取 capability（类型安全）
    pub fn get<C: Capability>(&self) -> Option<&C> {
        self.capabilities
            .get(&C::id())
            .and_then(|boxed| boxed.downcast_ref::<C>())
    }
}
```

### SDK 侧

```rust
impl Context {
    /// 获取 capability
    pub fn capability<C: Capability>(&self) -> Result<&C, CapabilityError> {
        self.registry
            .get::<C>()
            .ok_or(CapabilityError::NotFound {
                requested: C::name(),
                id: C::id(),
            })
    }
}
```

## 版本协商

Mod 声明所需的 capability 最低版本：

```toml
# manifest.toml
[capabilities]
event_bus = 1          # 需要 EventBus v1
commands = 1            # 需要 Commands v1（但 v1 没有 → 加载失败）

[capabilities.optional]
scheduler = 1           # 可选：有就用，没有就功能降级
```

Runtime 加载 mod 时：

```rust
fn negotiate_capabilities(
    manifest: &Manifest,
    registry: &CapabilityRegistry,
) -> Result<Vec<CapabilityId>, Vec<CapabilityError>> {
    let mut errors = Vec::new();

    // 必需 capability
    for (name, min_version) in &manifest.capabilities.required {
        let id = CapabilityId::from_name(name);
        match registry.version(id) {
            Some(v) if v >= min_version => {} // OK
            Some(v) => errors.push(CapabilityError::VersionMismatch {
                requested: min_version,
                available: v,
            }),
            None => errors.push(CapabilityError::NotFound { /* ... */ }),
        }
    }

    // 可选 capability
    let mut optional_available = Vec::new();
    for (name, min_version) in &manifest.capabilities.optional {
        let id = CapabilityId::from_name(name);
        match registry.version(id) {
            Some(v) if v >= min_version => optional_available.push(id),
            _ => {} // 可选的不满足，静默忽略
        }
    }

    if errors.is_empty() {
        Ok(optional_available)
    } else {
        Err(errors)
    }
}
```

## 演进策略

### v1 → v2 的 Capability 迁移

```
v1:
  CapabilityRegistry:
    - EventBus (v1)
    - ConfigStore (v1)

v2:
  CapabilityRegistry:
    - EventBus (v2, backward compat v1)
    - ConfigStore (v1)
    - CommandRegistry (v1, NEW)
    - Scheduler (v1, NEW)
```

老 mod 请求 EventBus v1 → Runtime 有 v2 → v2 >= v1 → OK ✅
新 mod 请求 Commands v1 → Runtime 有 v1 → OK ✅

## 设计原则

1. **Explicit** — 编译期知道依赖了哪些 capability
2. **Negotiable** — 运行时可检测 capability 是否存在
3. **Versioned** — capability 独立版本化，不跟 runtime 版本耦合
4. **Optional by design** — 文化上鼓励将新 capability 设为 optional，老 mod 不会因缺少新 cap 而加载失败
5. **No global state** — Capability 不是单例，所有访问走 Context
