//! Hello Morrow — demo mod (zero-copy, no String allocations).

use morrow::prelude::*;

static mut API: Option<RuntimeApi> = None;

#[morrow::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
    let a = unsafe { api.read() };
    // Read config
    let mut cbuf = [0u8; 512];
    let n = unsafe { (a.get_config)(0, b"hello-morrow".as_ptr(), 12, cbuf.as_mut_ptr(), cbuf.len() as u32) };
    if n > 0 { morrow::info!("Config: {} bytes", n); }
    // Capabilities
    for cap in [b"commands".as_slice(), b"host_api", b"config"] {
        let v = unsafe { (a.request_capability)(0, cap.as_ptr(), cap.len() as u32) };
        if v > 0 { morrow::info!("Cap {}: v{}", morrow::read_str(cap.as_ptr(), cap.len() as u32), v); }
    }
    unsafe { (a.register_command)(0, b"morrow".as_ptr(), 6, morrow_cmd); API = Some(a); }
    Ok(())
}

unsafe extern "C" fn morrow_cmd(_: *const u8, _: u32) {
    if let Some(ref api) = unsafe { API.as_ref() } {
        let mut buf = [0u8; 256];
        let n = unsafe { (api.get_player_list)(0, buf.as_mut_ptr(), buf.len() as u32) };
        let list = morrow::read_str(buf.as_ptr(), n);
        let t = unsafe { (api.get_world_time)(0) };
        let m = format!("Players: {}. Time: {}.", list, t);
        let b = m.as_bytes();
        unsafe { (api.send_message)(0, b.as_ptr(), b.len() as u32); }
    }
}

#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_server_start() { morrow::info!("Ready!"); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_server_stop() { morrow::info!("Bye!"); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_tick(t: u64) { if t % 200 == 0 { morrow::info!("tick {}", t); } }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_player_join(p: *const u8, l: u32) { morrow::info!("+ {}", morrow::read_str(p, l)); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_player_leave(p: *const u8, l: u32) { morrow::info!("- {}", morrow::read_str(p, l)); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_chat_message(a: *const u8, al: u32, b: *const u8, bl: u32) { morrow::info!("<{}> {}", morrow::read_str(a, al), morrow::read_str(b, bl)); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_block_break(a: *const u8, al: u32, b: *const u8, bl: u32) { morrow::info!("{} broke {}", morrow::read_str(a, al), morrow::read_str(b, bl)); }
#[unsafe(no_mangle)] pub extern "C" fn morrow_mod_player_death(p: *const u8, l: u32, _: *const u8, _: u32) { morrow::info!("{} died", morrow::read_str(p, l)); }
