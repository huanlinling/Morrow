//! motd — reads its welcome message from config.toml and greets every
//! joining player. Demonstrates typed config parsing with zero unsafe.

use morrow::prelude::*;
use serde::Deserialize;

/// Must mirror the keys in `config.toml` at the package root:
/// ```toml
/// message = "Welcome to the Morrow server!"
/// ```
#[derive(Deserialize)]
struct MotdConfig {
    message: String,
}

fn motd() -> String {
    morrow::config::<MotdConfig>()
        .ok()
        .flatten()
        .map(|c| c.message)
        .unwrap_or_else(|| "Welcome to the server!".to_string())
}

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    ctx.register_command("motd", motd_cmd)?;
    morrow::info!("MOTD: {}", motd());
    Ok(())
}

fn motd_cmd(_args: &str) {
    morrow::send_message(&motd());
}

#[morrow::event(player_join)]
fn on_join(player: &str) {
    morrow::send_message(&format!("{} — welcome, {}!", motd(), player));
}
