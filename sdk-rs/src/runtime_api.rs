//! Runtime API vtable — passed to mods during `morrow_mod_init`.

#[repr(C)]
pub struct RuntimeApi {
    pub get_player_count: unsafe extern "C" fn(runtime_handle: u64) -> i32,
    pub send_message: unsafe extern "C" fn(runtime_handle: u64, msg_ptr: *const u8, msg_len: u32),
    pub register_command: unsafe extern "C" fn(
        runtime_handle: u64,
        name_ptr: *const u8, name_len: u32,
        callback: unsafe extern "C" fn(*const u8, u32),
    ),
    pub get_player_list: unsafe extern "C" fn(runtime_handle: u64, buf: *mut u8, buf_cap: u32) -> u32,
    pub execute_command: unsafe extern "C" fn(runtime_handle: u64, cmd_ptr: *const u8, cmd_len: u32),
    pub get_world_time: unsafe extern "C" fn(runtime_handle: u64) -> i64,
    pub get_config: unsafe extern "C" fn(runtime_handle: u64, mod_name_ptr: *const u8, mod_name_len: u32, buf: *mut u8, buf_cap: u32) -> u32,
    pub request_capability: unsafe extern "C" fn(runtime_handle: u64, cap_ptr: *const u8, cap_len: u32) -> u32,
}
