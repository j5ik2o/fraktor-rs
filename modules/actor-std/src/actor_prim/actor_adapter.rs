use cellactor_actor_core_rs::error::ActorError;
use cellactor_utils_std_rs::StdToolbox;

use crate::{
  actor_prim::{Actor, ActorContext},
  messaging::AnyMessageView,
};

/// `ActorAdapter` bridges [`Actor`] implementations to the core runtime trait.
pub struct ActorAdapter<T> {
  inner: T,
}

impl<T> ActorAdapter<T> {
  /// Wraps the actor instance that will be registered in the runtime.
  #[must_use]
  pub const fn new(inner: T) -> Self {
    Self { inner }
  }
}

impl<T> cellactor_actor_core_rs::actor_prim::Actor<StdToolbox> for ActorAdapter<T>
where
  T: Actor,
{
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.inner.pre_start(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.inner.receive(ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.inner.post_stop(ctx)
  }
}
