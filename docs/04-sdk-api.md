# 04 — Rust SDK API 设计

## 设计目标

SDK 是对 Runtime Core ABI 的高层封装，目标：

1. **Ergonomic** — 开发者写 Rust，不写 FFI
2. **Safe** — 编译期尽可能多检查，运行时 catch_unwind
3. **Zero-cost abstraction** — 高层 API 编译后无额外开销
4. **Familiar** — 借鉴 Bevy ECS、Actix 等成熟 Rust 框架的 API 风格

## Mod 入口

### 最小 Mod

```rust
use ferrum::prelude::*;

// 定义 mod 结构体
struct MyMod {
    tick_count: u64,
}

// 实现 Mod 接口
impl FerrumMod for MyMod {
    fn metadata(&self) -> ModMetadata {
        ModMetadata::new("my-mod", "0.1.0")
            .description("My first Ferrum mod")
            .author("dev")
    }

    fn on_init(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
        ferrum::info!("MyMod initialized!");
        Ok(())
    }

    fn on_tick(&mut self, ctx: &mut Context, tick: u64) -> Result<(), FerrumError> {
        self.tick_count += 1;
        if tick % 20 == 0 {
            ferrum::info!("Second passed! Tick: {}", tick);
        }
        Ok(())
    }
}

// 导出为动态库入口
ferrum::export_mod!(MyMod);
```

### Proc Macro 版本（Milestone 5）

```rust
use ferrum::prelude::*;

#[ferrum::mod_main]
fn init(ctx: &mut Context) -> Result<(), FerrumError> {
    ctx.on_tick(|tick| {
        if tick % 20 == 0 {
            ferrum::info!("Tick: {}", tick);
        }
    });

    ctx.on_server_start(|_| {
        ferrum::info!("Server started!");
    });

    Ok(())
}
```

## Context API

`Context` 是 Mod 与 Runtime 交互的唯一入口：

```rust
pub struct Context {
    /// Capability 注册表
    capabilities: CapabilityRegistry,

    /// Event bus 引用
    event_bus: Option<Arc<EventBus>>,

    /// 当前 mod 的 metadata
    mod_meta: ModMetadata,

    /// Runtime 配置
    config: RuntimeConfig,
}

impl Context {
    /// 获取 capability（类型安全）
    pub fn capability<T: Capability>(&self) -> Result<&T, CapabilityError>;

    /// 获取 event bus
    pub fn event_bus(&self) -> Result<&EventBus, CapabilityError>;

    /// 获取命令注册表
    pub fn commands(&self) -> Result<&CommandRegistry, CapabilityError>;

    /// 读取配置
    pub fn config(&self) -> &RuntimeConfig;

    /// 获取其他已加载 mod 的信息
    pub fn mods(&self) -> Vec<ModInfo>;

    /// 记录日志（Java 侧 log4j 集成）
    pub fn log(&self, level: LogLevel, message: &str);
}
```

## Event API

```rust
/// 事件侦听器
pub struct EventListener<E: Event> {
    priority: EventPriority,
    handler: Box<dyn Fn(&E) + Send + Sync>,
}

/// Event trait — 所有事件实现此 trait
pub trait Event: Send + Sync + 'static {
    /// 事件类型标识符
    fn event_type() -> &'static str;
}

// ── 内置事件 ──

/// 服务端 tick
pub struct ServerTick {
    pub tick_number: u64,
}

impl Event for ServerTick {
    fn event_type() -> &'static str { "server.tick" }
}

/// 玩家加入
pub struct PlayerJoin {
    pub player_name: String,
    pub player_uuid: String,
}

impl Event for PlayerJoin {
    fn event_type() -> &'static str { "player.join" }
}

/// 玩家离开
pub struct PlayerLeave {
    pub player_name: String,
    pub player_uuid: String,
}

impl Event for PlayerLeave {
    fn event_type() -> &'static str { "player.leave" }
}
```

### 注册事件

```rust
impl EventBus {
    /// 注册事件监听器
    pub fn on<E: Event>(
        &self,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> EventListenerHandle;

    /// 带优先级
    pub fn on_with_priority<E: Event>(
        &self,
        priority: EventPriority,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> EventListenerHandle;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Lowest  = 0,
    Low     = 25,
    Normal  = 50,
    High    = 75,
    Highest = 100,
    Monitor = 200, // 最后运行，用于日志/监控，不可修改事件
}
```

## Command API (v2 preview)

```rust
impl CommandRegistry {
    /// 注册一个命令
    pub fn register(
        &self,
        name: &str,
        description: &str,
        handler: impl Fn(CommandContext) -> Result<(), CommandError> + Send + Sync + 'static,
    ) -> Result<(), CommandError>;
}

pub struct CommandContext {
    pub sender: CommandSender,
    pub args: Vec<String>,
}
```

## 日志 API

```rust
// 宏形式
ferrum::info!("Player {} joined", name);
ferrum::warn!("Low memory: {}MB remaining", mb);
ferrum::error!("Failed to save: {}", err);
ferrum::debug!("Position: {:?}", pos);
ferrum::trace!("Event dispatch took {}ns", ns);

// 会路由到 Java 侧的 log4j，与 Minecraft 日志统一
```

## 配置 API

```rust
// mod 可以在 manifest.toml 中声明默认配置
// 运行时 Java 侧会读取并传递给 Rust

#[derive(Deserialize)]
struct MyConfig {
    greeting: String,
    max_entities: u32,
}

fn on_init(&mut self, ctx: &mut Context) -> Result<(), FerrumError> {
    let config: MyConfig = ctx.config_for("my-mod")?;
    ferrum::info!("{}", config.greeting);
    Ok(())
}
```

## SDK 包结构

```
sdk-rs/
├── Cargo.toml
│   [dependencies]
│   ferrum-macros = { path = "../ferrum-macros" }
│   serde = { version = "1", features = ["derive"] }
│   serde_json = "1"
│   log = "0.4"
│
├── src/
│   ├── lib.rs            # pub mod prelude; re-exports
│   ├── prelude.rs        # 常用类型集中导入
│   ├── mod_trait.rs      # FerrumMod trait + derive
│   ├── context.rs        # Context 实现
│   ├── event.rs          # Event, EventBus, EventPriority
│   ├── command.rs        # CommandRegistry, CommandContext
│   ├── log.rs            # 日志宏
│   ├── config.rs         # 配置读取
│   └── error.rs          # FerrumError 类型
│
├── ferrum-macros/
│   ├── Cargo.toml
│   │   [lib]
│   │   proc-macro = true
│   │   [dependencies]
│   │   syn = "2"
│   │   quote = "1"
│   │   proc-macro2 = "1"
│   │
│   └── src/
│       ├── lib.rs         # proc macro 入口
│       └── mod_main.rs    # #[ferrum::mod_main] 实现
```

## SDK 设计原则

1. **trait 优于 macro** — 核心接口用 trait，macro 仅做糖
2. **显式优于隐式** — 所有 capability 显式获取，不做全局注入
3. **编译期检查优于运行时** — 类型安全的事件系统，handler 签名编译期匹配
4. **性能透明** — Context 方法调用开销可知（参考文档标注）
5. **渐进式抽象** — 可以用 proc macro 快速开发，也可以手写 trait 精细控制
