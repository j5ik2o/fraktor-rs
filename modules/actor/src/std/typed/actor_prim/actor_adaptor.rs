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
    // SAFETY: TypedActorContext is repr(transparent) wrapper around CoreTypedActorContextGeneric
    let wrapped_ctx =
      unsafe { &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>) };
    self.inner.pre_start(wrapped_ctx)
  }

  fn receive(
    &mut self,
    core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>,
    message: &M,
  ) -> Result<(), ActorError> {
    let wrapped_ctx =
      unsafe { &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>) };
    self.inner.receive(wrapped_ctx, message)
  }

  fn post_stop(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> Result<(), ActorError> {
    let wrapped_ctx =
      unsafe { &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>) };
    self.inner.post_stop(wrapped_ctx)
  }

  fn on_terminated(
    &mut self,
    core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>,
    terminated: Pid,
  ) -> Result<(), ActorError> {
    let wrapped_ctx =
      unsafe { &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>) };
    self.inner.on_terminated(wrapped_ctx, terminated)
  }

  fn supervisor_strategy(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> SupervisorStrategy {
    let wrapped_ctx =
      unsafe { &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>) };
    self.inner.supervisor_strategy(wrapped_ctx)
  }
}
