//! SDK 内部机制 — 不属于公共 API。
//!
//! 每个 mod 是独立 cdylib,本模块的 `static`/`thread_local` 都是
//! per-library 的,天然与其他 mod 隔离。

use crate::RuntimeApi;
use std::cell::Cell;
use std::sync::{LazyLock, Mutex};

// ---------------------------------------------------------------------------
// 当前 Runtime API(thread-local)
// ---------------------------------------------------------------------------
//
// 由 `#[morrow::mod_main]` 生成的 `morrow_mod_init` 在开头写入。
// 事件派发与 init 同在 Minecraft server 主线程(Mixin 注入点:
// loadWorld / tick / shutdown),自由函数与日志宏在此读取。

thread_local! {
    static CURRENT: Cell<Option<(*const RuntimeApi, &'static str)>> = const { Cell::new(None) };
}

#[doc(hidden)]
pub fn store_api(api: *const RuntimeApi, mod_name: &'static str) {
    CURRENT.with(|c| c.set(Some((api, mod_name))));
}

#[doc(hidden)]
pub fn with_api<R>(f: impl FnOnce(*const RuntimeApi) -> R) -> Option<R> {
    CURRENT.with(|c| c.get()).map(|(api, _)| f(api))
}

#[doc(hidden)]
pub fn current_mod_name() -> Option<&'static str> {
    CURRENT.with(|c| c.get()).map(|(_, n)| n)
}

// ---------------------------------------------------------------------------
// 日志:host log(转发 Java log4j),未设置时 fallback eprintln
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub fn log(level: u32, msg: &str) {
    if with_api(|api| unsafe { ((*api).log)(0, level, msg.as_ptr(), msg.len() as u32) }).is_none() {
        eprintln!("{}", msg);
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
