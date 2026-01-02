//! Typed ask response handle returned by `TypedActorRef::ask`.

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  messaging::AskResponseGeneric,
  typed::{actor::TypedActorRefGeneric, typed_ask_future::TypedAskFutureGeneric},
};

/// Associates the typed sender handle with the typed future.
///
/// The future resolves with `Ok(R)` on success, or `Err(TypedAskError)` on failure.
pub struct TypedAskResponseGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  sender: TypedActorRefGeneric<R, TB>,
  future: TypedAskFutureGeneric<R, TB>,
}

/// Type alias with the default toolbox.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, NoStdToolbox>;

impl<R, TB> TypedAskResponseGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) fn from_generic(response: AskResponseGeneric<TB>) -> Self {
    let (sender, future) = response.into_parts();
    let sender = TypedActorRefGeneric::from_untyped(sender);
    let future = TypedAskFutureGeneric::new(future);
    Self { sender, future }
  }

  /// Returns the sender target exposed to ask callers.
  #[must_use]
  pub const fn sender(&self) -> &TypedActorRefGeneric<R, TB> {
    &self.sender
  }

  /// Returns the typed future handle tied to this ask response.
  #[must_use]
  pub const fn future(&self) -> &TypedAskFutureGeneric<R, TB> {
    &self.future
  }

  /// Decomposes the response into its raw parts.
  #[must_use]
  pub fn into_parts(self) -> (TypedActorRefGeneric<R, TB>, TypedAskFutureGeneric<R, TB>) {
    (self.sender, self.future)
  }
}
