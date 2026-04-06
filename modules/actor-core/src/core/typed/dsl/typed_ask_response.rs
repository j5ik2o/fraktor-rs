//! Typed ask response handle returned by `TypedActorRef::ask`.

use crate::core::{
  kernel::actor::messaging::AskResponse,
  typed::{TypedActorRef, dsl::TypedAskFuture},
};

/// Associates the typed sender handle with the typed future.
///
/// The future resolves with `Ok(R)` on success, or `Err(TypedAskError)` on failure.
pub struct TypedAskResponse<R>
where
  R: Send + Sync + 'static, {
  sender: TypedActorRef<R>,
  future: TypedAskFuture<R>,
}

impl<R> TypedAskResponse<R>
where
  R: Send + Sync + 'static,
{
  pub(crate) fn from_generic(response: AskResponse) -> Self {
    let (sender, future) = response.into_parts();
    let sender = TypedActorRef::from_untyped(sender);
    let future = TypedAskFuture::new(future);
    Self { sender, future }
  }

  /// Returns the sender target exposed to ask callers.
  #[must_use]
  pub const fn sender(&self) -> &TypedActorRef<R> {
    &self.sender
  }

  /// Returns the typed future handle tied to this ask response.
  #[must_use]
  pub const fn future(&self) -> &TypedAskFuture<R> {
    &self.future
  }

  /// Decomposes the response into its raw parts.
  #[must_use]
  pub fn into_parts(self) -> (TypedActorRef<R>, TypedAskFuture<R>) {
    (self.sender, self.future)
  }
}
