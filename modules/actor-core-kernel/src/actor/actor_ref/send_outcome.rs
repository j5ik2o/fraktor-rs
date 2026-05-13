//! Result of a send operation, used to defer scheduling until locks are released.

use alloc::boxed::Box;
use core::fmt::{Debug, Formatter, Result as FmtResult};

/// Outcome returned by `ActorRefSender::send`.
pub enum SendOutcome {
  /// Message was delivered or enqueued; no further action required.
  Delivered,
  /// Additional work (e.g., dispatcher scheduling) that should run after caller releases locks.
  Schedule(Box<dyn FnOnce() + Send + 'static>),
}

impl Debug for SendOutcome {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | SendOutcome::Delivered => f.write_str("Delivered"),
      | SendOutcome::Schedule(_) => f.write_str("Schedule(<deferred>)"),
    }
  }
}
