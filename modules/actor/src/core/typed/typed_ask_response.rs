//! Typed ask response handle returned by `TypedActorRef::ask`.

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  messaging::AskResponseGeneric,
  typed::{actor_prim::TypedActorRefGeneric, typed_ask_future::TypedAskFutureGeneric},
};

/// Associates the typed reply handle with the typed future.
pub struct TypedAskResponseGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  reply_to: TypedActorRefGeneric<R, TB>,
  future:   TypedAskFutureGeneric<R, TB>,
}

/// Type alias with the default toolbox.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, NoStdToolbox>;

impl<R, TB> TypedAskResponseGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) fn from_generic(response: AskResponseGeneric<TB>) -> Self {
    let (reply_to, future) = response.into_parts();
    let reply_to = TypedActorRefGeneric::from_untyped(reply_to);
    let future = TypedAskFutureGeneric::new(future);
    Self { reply_to, future }
  }

  /// Returns the reply target exposed to ask callers.
  #[must_use]
  pub const fn reply_to(&self) -> &TypedActorRefGeneric<R, TB> {
    &self.reply_to
  }

  /// Returns the typed future handle tied to this ask response.
  #[must_use]
  pub const fn future(&self) -> &TypedAskFutureGeneric<R, TB> {
    &self.future
  }

  /// Decomposes the response into its raw parts.
  #[must_use]
  pub fn into_parts(self) -> (TypedActorRefGeneric<R, TB>, TypedAskFutureGeneric<R, TB>) {
    (self.reply_to, self.future)
  }
}
