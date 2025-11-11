//! User message snapshot captured when a failure occurs.

use core::{any::Any, ptr};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{RuntimeToolbox, actor_prim::Pid, messaging::AnyMessageGeneric};

/// Snapshot of the user message that triggered the failure.
#[derive(Clone)]
pub struct FailureMessageSnapshot {
  payload:  ArcShared<dyn Any + Send + Sync + 'static>,
  reply_to: Option<Pid>,
}

impl core::fmt::Debug for FailureMessageSnapshot {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("FailureMessageSnapshot").field("has_reply_to", &self.reply_to.is_some()).finish()
  }
}

impl FailureMessageSnapshot {
  /// Creates a snapshot from an owned payload pointer.
  #[must_use]
  pub const fn new(payload: ArcShared<dyn Any + Send + Sync + 'static>, reply_to: Option<Pid>) -> Self {
    Self { payload, reply_to }
  }

  /// Captures the payload/reply target information from a user message.
  #[must_use]
  pub fn from_message<TB: RuntimeToolbox>(message: &AnyMessageGeneric<TB>) -> Self {
    let payload = message.payload_arc();
    let reply_to = message.reply_to().map(|actor_ref| actor_ref.pid());
    Self { payload, reply_to }
  }

  /// Returns the stored reply-to pid, if any.
  #[must_use]
  pub const fn reply_to(&self) -> Option<Pid> {
    self.reply_to
  }

  /// Returns the raw payload Arc for diagnostics.
  #[must_use]
  pub fn payload(&self) -> ArcShared<dyn Any + Send + Sync + 'static> {
    self.payload.clone()
  }
}

impl PartialEq for FailureMessageSnapshot {
  fn eq(&self, other: &Self) -> bool {
    let left_ptr = ArcShared::into_raw(self.payload.clone());
    let right_ptr = ArcShared::into_raw(other.payload.clone());
    let equal = ptr::eq(left_ptr, right_ptr) && self.reply_to == other.reply_to;
    unsafe {
      let _ = ArcShared::from_raw(left_ptr);
      let _ = ArcShared::from_raw(right_ptr);
    }
    equal
  }
}

impl Eq for FailureMessageSnapshot {}
