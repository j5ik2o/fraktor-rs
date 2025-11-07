use cellactor_actor_core_rs::{actor_prim::Pid, error::ActorError, supervision::SupervisorStrategy};

use crate::typed::actor_prim::TypedActorContext;

/// Trait describing typed actors that can run on the standard runtime.
pub trait TypedActor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  /// Invoked once before the actor starts processing messages.
  ///
  /// # Errors
  /// Returns an error if the implementation fails to initialize actor state.
  fn pre_start(&mut self, _ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }
  /// Processes a single incoming message.
  ///
  /// # Errors
  /// Implementations return an error when message handling cannot complete successfully.
  fn receive(&mut self, _ctx: &mut TypedActorContext<'_, M>, _message: &M) -> Result<(), ActorError> {
    Ok(())
  }

  /// Runs after the actor has been stopped to allow custom cleanup.
  ///
  /// # Errors
  /// Return an error when cleanup fails and the system should treat it as actor failure.
  fn post_stop(&mut self, _ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Notifies the actor that one of its linked children terminated.
  ///
  /// # Errors
  /// Propagate an error when reacting to the termination cannot succeed.
  fn on_terminated(&mut self, _ctx: &mut TypedActorContext<'_, M>, _terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }

  /// Provides the supervision strategy for this typed actor.
  #[must_use]
  fn supervisor_strategy(&mut self, _ctx: &mut TypedActorContext<'_, M>) -> SupervisorStrategy {
    SupervisorStrategy::default()
  }
}
