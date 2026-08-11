//! Chat Bot — responds to chat (zero-copy, no String allocations).

use morrow::prelude::*;

static mut API: Option<RuntimeApi> = None;

#[morrow::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
    let a = unsafe { api.read() };
    unsafe { (a.register_command)(0, b"ping".as_ptr(), 4, ping); API = Some(a); }
    morrow::info!("Chat bot online! Say hi or /ping");
    Ok(())
}

unsafe extern "C" fn ping(_: *const u8, _: u32) {
    if let Some(ref api) = unsafe { API.as_ref() } {
        unsafe { (api.send_message)(0, b"Pong!".as_ptr(), 5); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_player_join(p: *const u8, l: u32) {
    let name = morrow::read_str(p, l);
    if let Some(ref api) = unsafe { API.as_ref() } {
        let m = format!("Welcome, {}!", name);
        unsafe { (api.send_message)(0, m.as_bytes().as_ptr(), m.len() as u32); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_chat_message(pl: *const u8, pll: u32, m: *const u8, ml: u32) {
    let player = morrow::read_str(pl, pll);
    let msg = morrow::read_str(m, ml);
    if let Some(ref api) = unsafe { API.as_ref() } {
        let reply = if msg.contains("hello") || msg.contains("hi") { Some("Hello!") }
            else if msg.contains("time") {
                let t = unsafe { (api.get_world_time)(0) };
                let r = format!("Time: {} ticks", t);
                return send(api, &r);
            } else { None };
        if let Some(r) = reply { send(api, &format!("{}: {}", player, r)); }
    }
}

fn send(api: &RuntimeApi, msg: &str) {
    unsafe { (api.send_message)(0, msg.as_bytes().as_ptr(), msg.len() as u32); }
}
