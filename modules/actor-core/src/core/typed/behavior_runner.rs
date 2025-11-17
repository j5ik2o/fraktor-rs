//! Executes typed behaviors inside the actor runtime.

use alloc::string::ToString;

use fraktor_utils_core_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  error::{ActorError, ActorErrorReason},
  event_stream::EventStreamEvent,
  supervision::SupervisorStrategy,
  typed::{
    UnhandledMessageEvent,
    actor_prim::{TypedActor, TypedActorContextGeneric},
    behavior::{Behavior, BehaviorDirective},
    behavior_signal::BehaviorSignal,
    message_adapter::{AdapterFailure, AdapterFailureEvent},
  },
};

#[cfg(test)]
mod tests;

/// Bridges [`Behavior`] objects with the [`TypedActor`] lifecycle.
pub(crate) struct BehaviorRunner<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  current:    Behavior<M, TB>,
  supervisor: Option<SupervisorStrategy>,
  stopping:   bool,
}

impl<M, TB> BehaviorRunner<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a runner with the provided initial behavior.
  pub(crate) fn new(initial: Behavior<M, TB>) -> Self {
    let supervisor = initial.supervisor_override().cloned();
    Self { current: initial, supervisor, stopping: false }
  }

  const fn update_supervisor_override(&mut self, strategy: Option<SupervisorStrategy>) {
    if let Some(strategy) = strategy {
      self.supervisor = Some(strategy);
    }
  }

  fn apply_transition(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    next: Behavior<M, TB>,
  ) -> Result<(), ActorError> {
    let override_strategy = next.supervisor_override().cloned();

    match next.directive() {
      | BehaviorDirective::Same | BehaviorDirective::Ignore => Ok(()),
      | BehaviorDirective::Unhandled => {
        // Keep the current behavior and emit UnhandledMessage event
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = UnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
        system.event_stream().publish(&EventStreamEvent::UnhandledMessage(event));
        Ok(())
      },
      | BehaviorDirective::Empty => {
        // Empty behavior: treat as unhandled and emit event, then keep empty behavior
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = UnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
        system.event_stream().publish(&EventStreamEvent::UnhandledMessage(event));
        self.current = Behavior::empty();
        Ok(())
      },
      | BehaviorDirective::Stopped => {
        if !self.stopping {
          ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
          self.stopping = true;
        }
        self.current = Behavior::stopped();
        Ok(())
      },
      | BehaviorDirective::Active => {
        self.current = next;
        Ok(())
      },
    }
    .map(|_| self.update_supervisor_override(override_strategy))
  }

  fn dispatch_signal(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    signal: &BehaviorSignal,
  ) -> Result<(), ActorError> {
    if let BehaviorSignal::AdapterFailed(failure) = signal {
      let event = AdapterFailureEvent::new(ctx.pid(), failure.clone());
      ctx.system().event_stream().publish(&EventStreamEvent::AdapterFailure(event));
    }
    let next = self.current.handle_signal(ctx, signal)?;
    self.apply_transition(ctx, next)
  }
}

impl<M, TB> TypedActor<M, TB> for BehaviorRunner<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn pre_start(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::Started)
  }

  fn receive(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>, message: &M) -> Result<(), ActorError> {
    let next = self.current.handle_message(ctx, message)?;
    self.apply_transition(ctx, next)
  }

  fn post_stop(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::Stopped)
  }

  fn on_terminated(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    terminated: crate::core::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::Terminated(terminated))
  }

  fn supervisor_strategy(&mut self, _ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> SupervisorStrategy {
    self.supervisor.clone().unwrap_or_default()
  }

  fn on_adapter_failure(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    failure: AdapterFailure,
  ) -> Result<(), ActorError> {
    let has_signal_handler = self.current.has_signal_handler();
    self.dispatch_signal(ctx, &BehaviorSignal::AdapterFailed(failure))?;
    if has_signal_handler {
      Ok(())
    } else {
      Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
    }
  }
}
