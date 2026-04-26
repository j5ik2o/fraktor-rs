//! Per-remote association state machine and its supporting types.
//!
//! This module is the pure, `&mut self` state machine that Pekko Artery's
//! `Association` (Scala, 1240 lines) maps to. I/O and scheduling live in the
//! `fraktor-remote-adaptor-std-rs` crate.

#[cfg(test)]
mod tests;

mod association_effect;
mod association_state;
mod base;
mod offer_outcome;
mod quarantine_reason;
mod send_queue;

pub use association_effect::AssociationEffect;
pub use association_state::AssociationState;
pub use base::Association;
pub use offer_outcome::OfferOutcome;
pub use quarantine_reason::QuarantineReason;
pub use send_queue::SendQueue;
