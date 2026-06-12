//! Typed ask response handle returned by `TypedActorRef::ask`.

#[cfg(test)]
#[path = "typed_ask_response_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::actor::messaging::AskResponse;

use crate::{TypedActorRef, dsl::TypedAskFuture};

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
    Self::from_untyped(response)
  }

  /// Wraps an untyped ask response with an asserted response type.
  ///
  /// This is the canonical conversion point for typed facade crates
  /// (such as `cluster-core-typed`) that need to attach a response type
  /// assertion to a raw [`AskResponse`] produced by the untyped kernel.
  ///
  /// # Note
  ///
  /// The response type `R` is asserted, not verified at construction time.
  /// A type mismatch is detected at call-site via
  /// [`TypedAskError::TypeMismatch`](crate::dsl::TypedAskError::TypeMismatch)
  /// when the caller attempts to take the resolved value.
  #[must_use]
  pub fn from_untyped(response: AskResponse) -> Self {
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
