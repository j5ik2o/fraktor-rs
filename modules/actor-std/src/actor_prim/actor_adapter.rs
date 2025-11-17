use fraktor_actor_core_rs::core::{actor_prim::Pid, error::ActorError};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  actor_prim::{Actor, ActorContext},
  messaging::AnyMessageView,
};

/// `ActorAdapter` bridges [`Actor`] implementations to the core runtime trait.
pub(crate) struct ActorAdapter<T> {
  inner: T,
}

impl<T> ActorAdapter<T> {
  /// Wraps the actor instance that will be registered in the runtime.
  #[must_use]
  pub(crate) const fn new(inner: T) -> Self {
    Self { inner }
  }
}

impl<T> fraktor_actor_core_rs::core::actor_prim::Actor<StdToolbox> for ActorAdapter<T>
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

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.inner.on_terminated(ctx, terminated)
  }
}
