//! Runtime API vtable — passed to mods during `ferrum_mod_init`.

/// Function table passed to the mod's entry point.
#[repr(C)]
pub struct RuntimeApi {
    /// Query the online player count via Java upcall.
    /// Pass `runtime_handle = 0` for the first available runtime.
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
    /// Send a message to the chat.
    pub send_message: unsafe extern "C" fn(runtime_handle: u64, msg_ptr: *const u8, msg_len: u32),
    /// Register a slash command.
    /// `callback(args_ptr, args_len)` is called when the command executes.
    pub register_command: unsafe extern "C" fn(
        runtime_handle: u64,
        name_ptr: *const u8, name_len: u32,
        callback: unsafe extern "C" fn(*const u8, u32),
    ),
}
