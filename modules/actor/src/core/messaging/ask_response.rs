//! Holds sender handle and future associated with an ask request.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  actor_prim::actor_ref::ActorRefGeneric, futures::ActorFutureSharedGeneric, messaging::AnyMessageGeneric,
};

/// Combines the sender handle and future returned by `ActorRefGeneric::ask`.
pub struct AskResponseGeneric<TB: RuntimeToolbox + 'static> {
  sender: ActorRefGeneric<TB>,
  future: ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for [AskResponseGeneric] with the default [NoStdToolbox].
pub type AskResponse = AskResponseGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskResponseGeneric<TB> {
  /// Creates a new ask response handle.
  #[must_use]
  pub const fn new(sender: ActorRefGeneric<TB>, future: ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>) -> Self {
    Self { sender, future }
  }

  /// Returns the sender handle exposed to the caller.
  #[must_use]
  pub const fn sender(&self) -> &ActorRefGeneric<TB> {
    &self.sender
  }

  /// Returns a reference to the shared future that resolves with the response message.
  #[must_use]
  pub const fn future(&self) -> &ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB> {
    &self.future
  }

  /// Decomposes the response into its parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorRefGeneric<TB>, ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>) {
    (self.sender, self.future)
  }
}
