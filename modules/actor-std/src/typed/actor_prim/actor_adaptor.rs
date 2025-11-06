use crate::typed::actor_prim::{TypedActor, TypedActorContext};
use cellactor_actor_core_rs::actor_prim::Pid;
use cellactor_actor_core_rs::error::ActorError;
use cellactor_actor_core_rs::typed::actor_prim::TypedActorContextGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub(crate) struct TypedActorAdapter<M, T> {
  inner: T,
  _marker: std::marker::PhantomData<M>,
}

impl<M, T> TypedActorAdapter<M, T> {
  /// Wraps the actor instance that will be registered in the runtime.
  #[must_use]
  pub(crate) const fn new(inner: T) -> Self {
    Self { inner, _marker: std::marker::PhantomData }
  }
}


impl<M, T> cellactor_actor_core_rs::typed::actor_prim::TypedActor<M, StdToolbox> for TypedActorAdapter<M, T>
where
  M: Send + Sync + 'static,
  T: TypedActor<M>,
{
  fn pre_start(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> Result<(), ActorError> {
    // SAFETY: TypedActorContext is repr(transparent) wrapper around CoreTypedActorContextGeneric
    let wrapped_ctx = unsafe {
      &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>)
    };
    self.inner.pre_start(wrapped_ctx)
  }

  fn receive(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>, message: &M) -> Result<(), ActorError> {
    let wrapped_ctx = unsafe {
      &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>)
    };
    self.inner.receive(wrapped_ctx, message)
  }

  fn post_stop(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>) -> Result<(), ActorError> {
    let wrapped_ctx = unsafe {
      &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>)
    };
    self.inner.post_stop(wrapped_ctx)
  }

  fn on_terminated(&mut self, core_ctx: &mut TypedActorContextGeneric<'_, M, StdToolbox>, terminated: Pid) -> Result<(), ActorError> {
    let wrapped_ctx = unsafe {
      &mut *(core_ctx as *mut TypedActorContextGeneric<'_, M, StdToolbox> as *mut TypedActorContext<'_, M>)
    };
    self.inner.on_terminated(wrapped_ctx, terminated)
  }
}
