extern crate std;

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::{
    actor_prim::Pid, error::ActorError, supervision::SupervisorStrategy, typed::actor_prim::TypedActorContextGeneric,
  },
  std::typed::actor_prim::{TypedActor, TypedActorContext},
};

/// Adapter bridging standard [`TypedActor`] implementations to the core runtime.
pub struct TypedActorAdapter<M, T> {
  inner:   T,
  _marker: std::marker::PhantomData<M>,
}

impl<M, T> TypedActorAdapter<M, T> {
  /// Wraps the actor instance that will be registered in the runtime.
  #[must_use]
  pub const fn new(inner: T) -> Self {
    Self { inner, _marker: std::marker::PhantomData }
  }
}

impl<M, T> crate::core::typed::TypedActor<M, StdToolbox> for TypedActorAdapter<M, T>
where
  M: Send + Sync + 'static,
  T: TypedActor<M>,
{
  fn pre_start(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> Result<(), ActorError> {
    let mut wrapped_ctx = TypedActorContext::from_core_mut(core_ctx);
    self.inner.pre_start(&mut wrapped_ctx)
  }

  fn receive(
    &mut self,
    core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>,
    message: &M,
  ) -> Result<(), ActorError> {
    let mut wrapped_ctx = TypedActorContext::from_core_mut(core_ctx);
    self.inner.receive(&mut wrapped_ctx, message)
  }

  fn post_stop(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> Result<(), ActorError> {
    let mut wrapped_ctx = TypedActorContext::from_core_mut(core_ctx);
    self.inner.post_stop(&mut wrapped_ctx)
  }

  fn on_terminated(
    &mut self,
    core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>,
    terminated: Pid,
  ) -> Result<(), ActorError> {
    let mut wrapped_ctx = TypedActorContext::from_core_mut(core_ctx);
    self.inner.on_terminated(&mut wrapped_ctx, terminated)
  }

  fn supervisor_strategy(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> SupervisorStrategy {
    let mut wrapped_ctx = TypedActorContext::from_core_mut(core_ctx);
    self.inner.supervisor_strategy(&mut wrapped_ctx)
  }
}
