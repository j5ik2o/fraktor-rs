//! Sender that rejects all messages.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{actor_prim::actor_ref::ActorRefSender, error::SendError, messaging::AnyMessageGeneric};

/// Sender that always returns a closed error.
#[derive(Default)]
pub struct NullSender;

impl<TB: RuntimeToolbox> ActorRefSender<TB> for NullSender {
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    Err(SendError::closed(message))
  }
}
