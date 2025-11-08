//! Executes typed behaviors inside the actor runtime.

use alloc::string::ToString;

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
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

#[cfg(test)]
mod tests {
  use alloc::{string::String, sync::Arc};
  use core::sync::atomic::{AtomicBool, Ordering};

  use cellactor_utils_core_rs::sync::NoStdToolbox;

  use super::BehaviorRunner;
  use crate::{
    actor_prim::ActorContextGeneric,
    system::ActorSystemGeneric,
    typed::{
      Behaviors,
      actor_prim::{TypedActor, TypedActorContextGeneric},
      behavior_signal::BehaviorSignal,
      message_adapter::{AdapterFailure, MessageAdapterRegistry},
    },
  };

  struct ProbeMessage;

  fn build_context() -> (ActorContextGeneric<'static, NoStdToolbox>, MessageAdapterRegistry<ProbeMessage, NoStdToolbox>)
  {
    let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
    let pid = system.allocate_pid();
    let ctx = ActorContextGeneric::new(&system, pid);
    (ctx, MessageAdapterRegistry::new())
  }

  #[test]
  fn behavior_runner_escalates_without_signal_handler() {
    let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
    let mut runner = BehaviorRunner::new(behavior);
    let (mut ctx, mut registry) = build_context();
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
    let result = runner.on_adapter_failure(&mut typed_ctx, AdapterFailure::Custom(String::from("boom")));
    assert!(result.is_err());
  }

  #[test]
  fn behavior_runner_allows_handled_adapter_failure() {
    let handled = Arc::new(AtomicBool::new(false));
    let witness = handled.clone();
    let behavior = Behaviors::receive_signal(move |_, signal| {
      if matches!(signal, BehaviorSignal::AdapterFailed(_)) {
        witness.store(true, Ordering::SeqCst);
      }
      Ok(Behaviors::same())
    });
    let mut runner = BehaviorRunner::new(behavior);
    let (mut ctx, mut registry) = build_context();
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
    let result = runner.on_adapter_failure(&mut typed_ctx, AdapterFailure::Custom(String::from("oops")));
    assert!(result.is_ok());
    assert!(handled.load(Ordering::SeqCst));
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
    terminated: crate::actor_prim::Pid,
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
    self.dispatch_signal(ctx, &BehaviorSignal::AdapterFailed(failure.clone()))?;
    if has_signal_handler {
      Ok(())
    } else {
      Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
    }
  }
}
