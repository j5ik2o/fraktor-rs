//! Result of a send operation, used to defer scheduling until locks are released.

use alloc::boxed::Box;

/// Outcome returned by `ActorRefSender::send`.
pub enum SendOutcome {
  /// Message was delivered or enqueued; no further action required.
  Delivered,
  /// Additional work (e.g., dispatcher scheduling) that should run after caller releases locks.
  Schedule(Box<dyn FnOnce() + Send + 'static>),
}

impl core::fmt::Debug for SendOutcome {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | SendOutcome::Delivered => f.write_str("Delivered"),
      | SendOutcome::Schedule(_) => f.write_str("Schedule(<deferred>)"),
    }
  }
}
