//! Holds reply handle and future associated with an ask request.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_prim::actor_ref::ActorRefGeneric, futures::ActorFuture,
  messaging::AnyMessageGeneric,
};

/// Combines the reply handle and future returned by `ActorRefGeneric::ask`.
pub struct AskResponseGeneric<TB: RuntimeToolbox + 'static> {
  reply_to: ActorRefGeneric<TB>,
  future:   ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>,
}

/// Type alias for [AskResponseGeneric] with the default [NoStdToolbox].
pub type AskResponse = AskResponseGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskResponseGeneric<TB> {
  /// Creates a new ask response handle.
  #[must_use]
  pub const fn new(reply_to: ActorRefGeneric<TB>, future: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) -> Self {
    Self { reply_to, future }
  }

  /// Returns the reply handle exposed to the caller.
  #[must_use]
  pub const fn reply_to(&self) -> &ActorRefGeneric<TB> {
    &self.reply_to
  }

  /// Returns a reference to the future that resolves with the response message.
  #[must_use]
  pub const fn future(&self) -> &ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>> {
    &self.future
  }

  /// Decomposes the response into its parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorRefGeneric<TB>, ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) {
    (self.reply_to, self.future)
  }
}
