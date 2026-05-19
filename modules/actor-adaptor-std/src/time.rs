//! Time bindings for the standard toolbox.

mod clock;
mod monotonic_mailbox_clock;

pub use clock::StdClock;
pub use monotonic_mailbox_clock::std_monotonic_mailbox_clock;
