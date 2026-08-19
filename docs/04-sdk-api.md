# 04 — Rust SDK 使用指南

SDK 是对 runtime ABI 的高层封装:开发者写普通 Rust,不写 FFI、不碰
裸指针。每个 mod 是一个独立 cdylib crate,依赖 `morrow`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
morrow = { path = "../../sdk-rs" }
```

## 最小 Mod

```rust
use morrow::prelude::*;

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    ctx.register_command("ping", ping)?; // Err(String) 经 ? 转 MorrowError
    Ok(())
}

fn ping(_args: &str) {
    morrow::send_message("Pong!");
}
```

打包后(`make package-hello` 或 `scripts/package-mod.sh <mod-dir>`),
runtime 会 dlopen 该库并调用生成的 `morrow_mod_init`。

## `#[morrow::mod_main]`

标记 mod 入口函数。支持两种签名:

| 签名 | 说明 |
|------|------|
| `fn(&mut Context) -> Result<(), MorrowError>` | 推荐 |
| `fn(&mut Context, *const RuntimeApi) -> Result<(), MorrowError>` | 旧式,透传原始 vtable |

宏生成 `extern "C" fn morrow_mod_init(*const RuntimeApi) -> u32`
(返回 0=成功, 1=失败),并:

1. 把 runtime API vtable 存入 per-library 全局 static(任何线程都能调
   用全局 API:写操作 `send_message`/`execute_command` 由 runtime 编组到
   主线程下一 tick 执行;读操作 `player_count`/`player_list`/`world_time`
   直达游戏,仅限主线程即事件/tick/命令处理器内调用)
2. 构造 `Context` 传入用户函数
3. 用 `catch_unwind` 包裹用户函数 — init 中 panic 不会穿过 FFI
   边界 abort,而是记录 `Init panicked: <msg>` 并以失败码返回

返回类型必须是 `Result<_, _>`;参数个数/类型不符时产生编译错误。

## 事件:`#[morrow::event(kind)]`

| kind | 导出符号 | Handler 签名 |
|------|----------|--------------|
| `tick` | `morrow_mod_tick` | `fn(u64)` |
| `server_start` | `morrow_mod_server_start` | `fn()` |
| `server_stop` | `morrow_mod_server_stop` | `fn()` |
| `player_join` / `player_leave` | `morrow_mod_player_join` / `_leave` | `fn(&str)` |
| `chat_message` / `block_break` / `block_place` / `player_death` | `morrow_mod_<kind>` | `fn(&str, &str)` |

```rust
#[morrow::event(chat_message)]
fn on_chat(player: &str, msg: &str) {
    morrow::info!("<{}> {}", player, msg);
}

#[morrow::event(tick)]
fn on_tick(t: u64) {
    if t % 200 == 0 {
        morrow::info!("tick {}", t);
    }
}
```

- Handler 保持普通 Rust 签名;宏生成 `#[unsafe(no_mangle)] extern "C"`
  导出,内部用 `read_str` 零拷贝解包。
- 参数个数/类型不匹配会得到带位置的编译错误。
- **每 mod 每 kind 至多一个 handler** — ABI 是符号发现制,重复
  kind 会以 duplicate symbol 链接错误暴露。
- 事件派发在 Minecraft server 主线程,与 init 同一线程。

## Context

`Context` 是 `Copy` 的,需要持有时(如事件 handler 中调用 API)可在
init 里拷贝存入静态变量:

```rust
static CTX: OnceLock<Context> = OnceLock::new();

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    CTX.set(*ctx).unwrap();
    Ok(())
}
```

不要手动构造 `Context` — api 指针必须来自宏生成的 init。

| 方法 | 返回 | 说明 |
|------|------|------|
| `send_message(&str)` | — | 全体玩家聊天广播 |
| `execute_command(&str)` | — | 执行服务端命令 |
| `player_count()` | `i32` | 在线人数;host API 未注册时为 -1 |
| `player_list()` | `Vec<String>` | 在线玩家名 |
| `world_time()` | `i64` | 世界时间(ticks);未注册时为 -1 |
| `register_command(name, fn(&str))` | `Result<(), String>` | 注册聊天命令;命令名已被其他 mod 占用或槽位池满(64/每 mod)时返回 Err,槽位自动归还 |
| `config_raw()` | `Option<String>` | 本 mod 的 config.toml 原文(≤4096 字节) |
| `config::<T>()` | `Result<Option<T>, String>` | 解析为类型化结构,见[配置](#配置) |
| `request_capability(&str)` | `u32` | 能力版本;0 = 不可用 |
| `log(LogLevel, &str)` | — | 经 host 转发到 Minecraft log4j |

## 事件内的全局函数

事件 handler 里可以直接用 crate 根自由函数。API vtable 存在全局
static 中(init 后任何线程可用);在 init 之前调用会 **panic 并带
明确消息**(显式错误优于静默 no-op),唯一例外是 [`log`] — 它永远
可用(fallback 到 stderr):

```rust
morrow::send_message("...");
morrow::execute_command("say hi");
morrow::player_count();
morrow::player_list();     // Vec<String>
morrow::world_time();
morrow::config::<MyCfg>(); // Result<Option<MyCfg>, String>
morrow::config_raw();      // Option<String>
morrow::log(LogLevel::Warn, "...");
```

## 日志宏

```rust
morrow::info!("Player {} joined", name);
morrow::warn!("Low memory: {}MB", mb);
morrow::error!("Failed: {}", err);
```

- 走 host 日志通道(level 1/2/3),最终进 Minecraft 日志;
  init 之前调用则 fallback 到 stderr。
- 自动加 `[mod-name]` 前缀。
- **必须用显式参数** — 宏展开的 `format!` 不支持隐式捕获,
  `info!("{name}")` 会编译报错,请写 `info!("{}", name)`。

## 配置

- 打包时把 `config.toml` 放进 mod 目录即随 `.morrow` 包分发。
- 读取按 **cargo 包名**(`CARGO_PKG_NAME`)键控 — 需与
  `manifest.toml` 的 `[package] name` 一致。
- 推荐类型化读取:`ctx.config::<T>()`,结构体 derive
  `serde::Deserialize` 并镜像 config.toml 的键。无 config.toml 时
  返回 `Ok(None)`,解析失败返回带 TOML 行号的错误消息。
- 需要原文时用 `ctx.config_raw()`(≤4096 字节)。

```rust
#[derive(serde::Deserialize)]
struct Cfg { message: String, interval_seconds: u32 }

let cfg: Cfg = ctx.config()?.unwrap_or(Cfg {
    message: "Welcome!".into(),
    interval_seconds: 60,
});
```

## 命令

- `ctx.register_command(name, handler)`,handler 收到参数串(可为空)。
- 底层 ABI 回调无 userdata,SDK 用预生成的 64 个 trampoline 槽位
  把命令名映射到 handler —— **每 mod 最多 64 个命令**。
- 命令名冲突(runtime 层全局命令表)与池满都会让
  `register_command` 返回 `Err`,且失败的槽位会被归还 — 换名重试
  依然可用。冲突不覆盖:先注册的 mod 保持所有权。
- 命令 handler 可以自由调用全局 API(`send_message` 等)— runtime
  在调用 handler 前已释放所有锁(runtime 内部按快照模式派发)。

## ABI 约束(开发者须知)

- 事件分发是**符号发现制**:runtime 在 dlopen 后按固定符号名+签名
  查找,宏负责生成正确符号。手写导出时请对照上表。
- API 调用一律传 handle `0`(= "any live runtime")。
- `morrow_mod_init` 的符号签名 `extern "C" fn(*const RuntimeApi) -> u32`
  不可变(runtime 按此签名加载)。

## 示例

| 示例 | 演示 |
|------|------|
| `examples/hello-morrow` | 全 API 演示:config、capability、命令、9 种事件 |
| `examples/chat-bot` | 聊天回应 + `/ping`,player_join 欢迎 |
| `examples/motd` | config.toml 读取 + 进服欢迎 + `/motd` 命令 |
