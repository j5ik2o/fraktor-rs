use crate::{
  core::{
    actor::{ActorContext, Pid},
    error::ActorError,
    messaging::AnyMessageView,
    supervision::SupervisorStrategyConfig,
  },
  std::actor::{Actor, ActorContext as StdActorContext},
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

impl<T> crate::core::actor::Actor for ActorAdapter<T>
where
  T: Actor,
{
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let mut wrapped_ctx = StdActorContext::from_core_mut(ctx);
    self.inner.pre_start(&mut wrapped_ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    let mut wrapped_ctx = StdActorContext::from_core_mut(ctx);
    self.inner.receive(&mut wrapped_ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let mut wrapped_ctx = StdActorContext::from_core_mut(ctx);
    self.inner.post_stop(&mut wrapped_ctx)
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    let mut wrapped_ctx = StdActorContext::from_core_mut(ctx);
    self.inner.on_terminated(&mut wrapped_ctx, terminated)
  }

  fn supervisor_strategy(&self, ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    let wrapped_ctx = StdActorContext::from_core_mut(ctx);
    self.inner.supervisor_strategy(&wrapped_ctx)
  }
}
