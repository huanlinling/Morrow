//! Integration-test mod. Built as a cdylib, packaged into a `.mor`
//! zip by `runtime-rs/tests/mod_loader_integration.rs`, and loaded
//! through the real loader. Every behavior leaves a trace in the host
//! log (`morrow::info!`) or a `send_message`, which the test host
//! vtable collects and asserts on.

use morrow::prelude::*;

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    morrow::info!("init-ok");
    ctx.register_command("testmod_ping", ping)?;
    ctx.register_command("testmod_count", count)?;
    Ok(())
}

fn ping(args: &str) {
    morrow::send_message(&format!("pong:{args}"));
}

fn count(args: &str) {
    let n = morrow::player_count();
    morrow::send_message(&format!("players={n};args={args}"));
}

#[morrow::event(server_start)]
fn on_server_start() {
    morrow::info!("server-start");
}

#[morrow::event(server_stop)]
fn on_server_stop() {
    morrow::info!("server-stop");
}

#[morrow::event(tick)]
fn on_tick(t: u64) {
    if t == 42 {
        morrow::info!("tick-42");
    }
}

#[morrow::event(player_join)]
fn on_join(player: &str) {
    morrow::info!("join:{}", player);
}

#[morrow::event(player_leave)]
fn on_leave(player: &str) {
    morrow::info!("leave:{}", player);
}

#[morrow::event(chat_message)]
fn on_chat(player: &str, msg: &str) {
    morrow::info!("chat:{}:{}", player, msg);
}

#[morrow::event(block_break)]
fn on_block_break(player: &str, block: &str) {
    morrow::info!("break:{}:{}", player, block);
}

#[morrow::event(block_place)]
fn on_block_place(player: &str, block: &str) {
    morrow::info!("place:{}:{}", player, block);
}

#[morrow::event(player_death)]
fn on_death(player: &str, cause: &str) {
    morrow::info!("death:{}:{}", player, cause);
}
