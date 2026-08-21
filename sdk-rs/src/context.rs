//! `Context` — mod 与 runtime 交互的安全窗口。
//!
//! 由 `#[morrow::mod_main]` 生成的 `morrow_mod_init` 构造并传给用户
//! 函数。`Context` 是 `Copy` 的:需要持有时(例如在事件 handler 中调用
//! API)可以在 init 里拷贝一份存入 `static OnceLock<Context>`。
//!
//! ```ignore
//! static CTX: OnceLock<Context> = OnceLock::new();
//!
//! #[morrow::mod_main]
//! fn init(ctx: &mut Context) -> Result<(), MorrowError> {
//!     CTX.set(*ctx).unwrap();
//!     Ok(())
//! }
//! ```
//!
//! 注意:不要手动构造 `Context` — `api` 指针必须来自 runtime 的
//! `morrow_mod_init`(恒非空)。

use crate::runtime_api::RuntimeApi;
use crate::{__internal, LogLevel};

/// Execution context for a Morrow mod.
#[derive(Clone, Copy)]
pub struct Context {
    api: *const RuntimeApi,
    /// 恒为 0 = "any live runtime"(mod API 约定)。
    handle: u64,
    /// crate 名 — config 按此键控,需与 manifest package name 一致。
    mod_name: &'static str,
}

// vtable 指向 runtime 进程内全局函数,内部全部 Mutex 同步 + panic 隔离,
// 跨线程共享是 sound 的。
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl Context {
    /// 仅由宏生成的 `morrow_mod_init` 调用。
    #[doc(hidden)]
    pub fn from_api(api: *const RuntimeApi, mod_name: &'static str) -> Self {
        Context {
            api,
            handle: 0,
            mod_name,
        }
    }

    /// Broadcast a message to all players' chat.
    pub fn send_message(&self, msg: &str) {
        unsafe { (self.api.read().send_message)(self.handle, msg.as_ptr(), msg.len() as u32) }
    }

    /// Run a server command (e.g. `/say hi`).
    pub fn execute_command(&self, cmd: &str) {
        unsafe { (self.api.read().execute_command)(self.handle, cmd.as_ptr(), cmd.len() as u32) }
    }

    /// Online player count. Returns -1 if the host API is not registered.
    pub fn player_count(&self) -> i32 {
        unsafe { (self.api.read().get_player_count)(self.handle) }
    }

    /// Online player names.
    pub fn player_list(&self) -> Vec<String> {
        // Matches the runtime's 64 KiB snapshot buffer ceiling.
        let mut buf = vec![0u8; 65536];
        let n = unsafe {
            (self.api.read().get_player_list)(self.handle, buf.as_mut_ptr(), buf.len() as u32)
        };
        crate::parse_player_list(crate::read_str(buf.as_ptr(), n))
    }

    /// World time in ticks. Returns -1 if the host API is not registered.
    pub fn world_time(&self) -> i64 {
        unsafe { (self.api.read().get_world_time)(self.handle) }
    }

    /// Register a chat command. The handler receives the argument string
    /// (possibly empty).
    ///
    /// Errors: command pool full (64 commands per mod) or the name is
    /// already registered by another mod (runtime-level conflict). On
    /// error the pool slot is released, so retry with a different name
    /// still works.
    pub fn register_command(&self, name: &str, handler: fn(&str)) -> Result<(), String> {
        let Some(trampoline) = __internal::register_command_slot(handler) else {
            return Err(format!(
                "command pool exhausted (max {} commands per mod)",
                __internal::COMMAND_SLOT_COUNT
            ));
        };
        let status = unsafe {
            (self.api.read().register_command)(
                self.handle,
                name.as_ptr(),
                name.len() as u32,
                trampoline,
            )
        };
        if status != 0 {
            __internal::unregister_command_slot(trampoline);
            return Err(format!("command '/{name}' already registered"));
        }
        Ok(())
    }

    /// Read the mod's config.toml as raw TOML text (≤ 4096 bytes).
    /// None when the package has no config.toml.
    pub fn config_raw(&self) -> Option<String> {
        let mut buf = [0u8; 4096];
        let n = unsafe {
            (self.api.read().get_config)(
                self.handle,
                self.mod_name.as_ptr(),
                self.mod_name.len() as u32,
                buf.as_mut_ptr(),
                buf.len() as u32,
            )
        };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    }

    /// Read and parse the mod's config.toml into a typed struct.
    ///
    /// Returns `Ok(None)` when the package has no config.toml, `Ok(Some(..))`
    /// on success, and an error message on parse failure or a malformed
    /// schema — mod authors see exactly what TOML line failed to parse.
    ///
    /// ```ignore
    /// #[derive(serde::Deserialize)]
    /// struct Cfg { message: String, interval_seconds: u32 }
    ///
    /// let cfg: Cfg = ctx.config()?.unwrap_or(Cfg { ..defaults });
    /// ```
    pub fn config<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>, String> {
        self.config_raw()
            .map(|raw| {
                toml::from_str(&raw).map_err(|e| format!("config.toml: {e}"))
            })
            .transpose()
    }

    /// Request a capability version. Returns 0 if unavailable.
    pub fn request_capability(&self, cap: &str) -> u32 {
        unsafe {
            (self.api.read().request_capability)(self.handle, cap.as_ptr(), cap.len() as u32)
        }
    }

    /// Log through the host (forwarded to Minecraft's log4j).
    pub fn log(&self, level: LogLevel, msg: &str) {
        __internal::log(level as u32, msg)
    }
}
