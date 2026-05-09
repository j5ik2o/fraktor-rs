//! Panic-catching invoke guard for std-enabled actor systems.

use alloc::string::{String, ToString};
use core::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_actor_core_rs::actor::{
  error::{ActorError, ActorErrorReason},
  invoke_guard::InvokeGuard,
};

/// Converts panics raised during `receive` into escalation errors.
pub struct PanicInvokeGuard;

impl PanicInvokeGuard {
  /// Creates a panic-catching guard.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
      return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
      return message.clone();
    }
    String::from("non-string panic payload")
  }
}

impl Default for PanicInvokeGuard {
  fn default() -> Self {
    Self::new()
  }
}

impl InvokeGuard for PanicInvokeGuard {
  fn wrap_receive(&self, call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError> {
    match catch_unwind(AssertUnwindSafe(call)) {
      | Ok(result) => result,
      | Err(payload) => {
        Err(ActorError::escalate(ActorErrorReason::new(format!("panic: {}", Self::panic_message(payload.as_ref())))))
      },
    }
  }
}
