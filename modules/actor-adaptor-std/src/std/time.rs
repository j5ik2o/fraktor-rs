//! Time bindings for the standard toolbox.

mod std_clock;
mod std_mailbox_clock;

pub use std_clock::StdClock;
pub use std_mailbox_clock::std_monotonic_mailbox_clock;
