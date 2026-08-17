//! M7 scalability fixture — a mod that does nothing.
//!
//! The tick handler is deliberately silent (no log, no upcall) so the
//! scalability test measures pure dispatch overhead: registry iteration
//! + fn call + catch_unwind per mod.

use morrow::prelude::*;

#[morrow::mod_main]
fn init(_ctx: &mut Context) -> Result<(), MorrowError> {
    Ok(())
}

#[morrow::event(tick)]
fn on_tick(_t: u64) {}
