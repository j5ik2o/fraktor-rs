//! Minimal PersistentFSM-compatible layer on top of `PersistentActor`.

#[cfg(test)]
mod tests;

use core::any::Any;

use fraktor_actor_rs::core::actor::ActorContextGeneric;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::persistent_actor::PersistentActor;

/// Minimal PersistentFSM-compatible contract.
///
/// Implementors keep the FSM state and domain state inside the actor and define how
/// persisted domain events are applied.
pub trait PersistentFsm<TB: RuntimeToolbox + 'static>: PersistentActor<TB> {
  /// FSM state type.
  type State: Send + Sync + 'static;

  /// Domain event type persisted by this FSM.
  type DomainEvent: Any + Send + Sync + 'static;

  /// Applies a persisted domain event to the current actor state.
  fn apply_fsm_event(&mut self, event: &Self::DomainEvent);

  /// Replaces the current FSM state.
  fn set_fsm_state(&mut self, state: Self::State);

  /// Returns the current FSM state.
  fn fsm_state(&self) -> &Self::State;

  /// Persists an event and applies the state transition after persistence acknowledgment.
  fn persist_state_transition(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    event: Self::DomainEvent,
    next_state: Self::State,
  ) {
    self.persist(ctx, event, move |actor, persisted_event| {
      actor.apply_fsm_event(persisted_event);
      actor.set_fsm_state(next_state);
    });
  }

  /// Persists an event asynchronously and applies the state transition after persistence
  /// acknowledgment.
  fn persist_state_transition_async(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    event: Self::DomainEvent,
    next_state: Self::State,
  ) {
    self.persist_async(ctx, event, move |actor, persisted_event| {
      actor.apply_fsm_event(persisted_event);
      actor.set_fsm_state(next_state);
    });
  }
}
