//! SDK 内部机制 — 不属于公共 API。
//!
//! 每个 mod 是独立 cdylib,本模块的 `static` 都是 per-library 的,
//! 天然与其他 mod 隔离。

use crate::RuntimeApi;
use std::sync::{LazyLock, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// 当前 Runtime API(全局 static)
// ---------------------------------------------------------------------------
//
// 由 `#[morrow::mod_main]` 生成的 `morrow_mod_init` 在开头写入。
//
// 为什么是全局 static 而不是 thread_local:runtime 传入的 vtable 是
// 进程内全局静态函数表(runtime 侧 `Box::leak` 保证 'static),本身与
// 线程无关。用全局 static 后,mod 自 spawn 的线程也能调用全局 API
// (send_message 等)并正确工作 — thread_local 版本里那些调用会静默
// no-op,是隐藏 bug 的来源。每个 mod 是独立 cdylib,这里的 static 是
// 该库私有的,不会跨 mod 串扰。
//
// 错误语义:未初始化(init 之外、或库未被 runtime 加载)时,消息类
// 自由函数显式 panic(带定位信息)而不是静默丢弃;唯一例外是日志,
// 它永远可用(fallback 到 stderr),因为日志在错误路径上更要工作。

static API: OnceLock<(&'static RuntimeApi, &'static str)> = OnceLock::new();

#[doc(hidden)]
pub fn store_api(api: *const RuntimeApi, mod_name: &'static str) {
    // vtable 由 runtime 泄漏,进程存活期间恒有效,转 &'static 是 sound 的。
    let api_ref: &'static RuntimeApi = unsafe { &*api };
    // 同一库被重复 init(罕见)时保留第一个 — 值等价,无实际影响。
    let _ = API.set((api_ref, mod_name));
}

/// 当前 `RuntimeApi` vtable。未初始化时 panic — 在 init 之外调用全局
/// API 是编程错误,显式失败优于静默 no-op。
#[doc(hidden)]
#[inline]
pub fn api() -> &'static RuntimeApi {
    API.get()
        .expect(
            "Morrow SDK: runtime API not initialized — \
             call this from a #[morrow::mod_main] init or an event handler",
        )
        .0
}

/// 当前 mod 名(config 按此键控)。未初始化时 panic,同上。
#[doc(hidden)]
#[inline]
pub fn current_mod_name() -> &'static str {
    API.get()
        .expect(
            "Morrow SDK: runtime API not initialized — \
             call this from a #[morrow::mod_main] init or an event handler",
        )
        .1
}

// ---------------------------------------------------------------------------
// 日志:host log(转发 Java log4j),未设置时 fallback eprintln
// ---------------------------------------------------------------------------
//
// 刻意不 panic:日志在错误路径上是最需要的工具,永远可用。

#[doc(hidden)]
pub fn log(level: u32, msg: &str) {
    match API.get() {
        Some((api, _)) => unsafe { (api.log)(0, level, msg.as_ptr(), msg.len() as u32) },
        None => eprintln!("{}", msg),
    }
}

// ---------------------------------------------------------------------------
// 命令 trampoline 槽位池
// ---------------------------------------------------------------------------
//
// runtime 的回调 ABI 是 `fn(*const u8, u32)`:没有 userdata,派发时也不
// 回传命令名,所以一个共享 trampoline 无法知道要调哪个 handler — 每个
// 命令必须有一个独立的 C 函数。Rust 无法在运行时生成函数,因此预生成
// 64 个 const-generic trampoline,注册时取第一个空槽。

pub(crate) const COMMAND_SLOT_COUNT: usize = 64;

static COMMAND_SLOTS: LazyLock<Mutex<Vec<Option<fn(&str)>>>> = LazyLock::new(|| {
    Mutex::new((0..COMMAND_SLOT_COUNT).map(|_| None).collect())
});

extern "C" fn command_trampoline<const I: usize>(ptr: *const u8, len: u32) {
    // Option<fn(&str)> 是 Copy,锁在语句结束即释放 — handler 内再注册命令不会死锁。
    let handler = COMMAND_SLOTS.lock().unwrap()[I];
    if let Some(h) = handler {
        h(crate::read_str(ptr, len));
    }
}

macro_rules! trampoline_ptrs {
    ($($i:literal),* $(,)?) => {
        [$(command_trampoline::<$i> as unsafe extern "C" fn(*const u8, u32)),*]
    };
}

pub(crate) static COMMAND_TRAMPOLINES: [unsafe extern "C" fn(*const u8, u32); COMMAND_SLOT_COUNT] =
    trampoline_ptrs!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
        32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63
    );

/// 分配一个命令槽,返回对应的 trampoline。池满时返回 None。
#[doc(hidden)]
pub fn register_command_slot(handler: fn(&str)) -> Option<unsafe extern "C" fn(*const u8, u32)> {
    let mut slots = COMMAND_SLOTS.lock().unwrap();
    let idx = slots.iter().position(|s| s.is_none())?;
    slots[idx] = Some(handler);
    Some(COMMAND_TRAMPOLINES[idx])
}

/// 归还一个命令槽(注册被 runtime 拒绝时)。返回是否找到了该槽。
#[doc(hidden)]
pub fn unregister_command_slot(
    trampoline: unsafe extern "C" fn(*const u8, u32),
) -> bool {
    let mut slots = COMMAND_SLOTS.lock().unwrap();
    match COMMAND_TRAMPOLINES
        .iter()
        .position(|t| std::ptr::fn_addr_eq(*t, trampoline))
    {
        Some(idx) if slots[idx].is_some() => {
            slots[idx] = None;
            true
        }
        _ => false,
    }
}
