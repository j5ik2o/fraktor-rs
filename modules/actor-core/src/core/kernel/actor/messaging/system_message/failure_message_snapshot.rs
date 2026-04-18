//! User message snapshot captured when a failure occurs.

use core::{
  any::Any,
  fmt::{Debug, Formatter, Result as FmtResult},
  ptr,
};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::actor::{Pid, messaging::AnyMessage};

/// Snapshot of the user message that triggered the failure.
#[derive(Clone)]
pub struct FailureMessageSnapshot {
  payload: ArcShared<dyn Any + Send + Sync + 'static>,
  sender:  Option<Pid>,
}

impl Debug for FailureMessageSnapshot {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("FailureMessageSnapshot").field("has_sender", &self.sender.is_some()).finish()
  }
}

impl FailureMessageSnapshot {
  /// Creates a snapshot from an owned payload pointer.
  #[must_use]
  pub const fn new(payload: ArcShared<dyn Any + Send + Sync + 'static>, sender: Option<Pid>) -> Self {
    Self { payload, sender }
  }

  /// Captures the payload/sender information from a user message.
  #[must_use]
  pub fn from_message(message: &AnyMessage) -> Self {
    let payload = message.payload_arc();
    let sender = message.sender().map(|actor_ref| actor_ref.pid());
    Self { payload, sender }
  }

  /// Returns the stored sender pid, if any.
  #[must_use]
  pub const fn sender(&self) -> Option<Pid> {
    self.sender
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
    let equal = ptr::eq(left_ptr, right_ptr) && self.sender == other.sender;
    unsafe {
      // into_raw で増やした参照カウントを戻すために from_raw の戻り値を即座に drop する。
      drop(ArcShared::from_raw(left_ptr));
      drop(ArcShared::from_raw(right_ptr));
    }
    equal
  }
}

impl Eq for FailureMessageSnapshot {}
