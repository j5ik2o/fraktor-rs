//! Adapts typed actors to the untyped runtime.

use alloc::boxed::Box;

use crate::{
  RuntimeToolbox,
  actor_prim::{Actor, ActorContextGeneric},
  error::{ActorError, ActorErrorReason},
  messaging::AnyMessageView,
  typed::actor_prim::{TypedActor, TypedActorContextGeneric},
};

const DOWNCAST_FAILED: &str = "typed actor received unexpected message";

/// Wraps a typed actor and exposes the untyped [`Actor`] interface.
pub(crate) struct TypedActorAdapter<TB, M>
where
  TB: RuntimeToolbox + 'static,
  M: Send + Sync + 'static, {
  actor: Box<dyn TypedActor<M, TB>>,
}

impl<TB, M> TypedActorAdapter<TB, M>
where
  TB: RuntimeToolbox + 'static,
  M: Send + Sync + 'static,
{
  /// Creates a new adapter from the provided typed actor.
  #[must_use]
  pub(crate) fn new<A>(actor: A) -> Self
  where
    A: TypedActor<M, TB> + 'static, {
    Self { actor: Box::new(actor) }
  }
}

impl<TB, M> Actor<TB> for TypedActorAdapter<TB, M>
where
  TB: RuntimeToolbox + 'static,
  M: Send + Sync + 'static,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx);
    self.actor.pre_start(&mut typed_ctx)
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageView<'_, TB>,
  ) -> Result<(), ActorError> {
    let payload =
      message.downcast_ref::<M>().ok_or_else(|| ActorError::recoverable(ActorErrorReason::new(DOWNCAST_FAILED)))?;
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx);
    self.actor.receive(&mut typed_ctx, payload)
  }

  fn post_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx);
    self.actor.post_stop(&mut typed_ctx)
  }

  fn on_terminated(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    terminated: crate::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx);
    self.actor.on_terminated(&mut typed_ctx, terminated)
  }
}
