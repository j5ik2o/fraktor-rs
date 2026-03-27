//! Executes typed behaviors inside the actor runtime.

use alloc::string::ToString;

use crate::core::{
  error::{ActorError, ActorErrorReason},
  event::stream::{AdapterFailureEvent, EventStreamEvent, TypedUnhandledMessageEvent},
  supervision::SupervisorStrategyConfig,
  typed::{
    DeathPactException,
    actor::{TypedActor, TypedActorContext},
    behavior::{Behavior, BehaviorDirective},
    behavior_signal::BehaviorSignal,
    message_adapter::AdapterError,
  },
};

#[cfg(test)]
mod tests;

/// Bridges [`Behavior`] objects with the [`TypedActor`] lifecycle.
pub(crate) struct BehaviorRunner<M>
where
  M: Send + Sync + 'static, {
  current:    Behavior<M>,
  supervisor: Option<SupervisorStrategyConfig>,
  stopping:   bool,
}

impl<M> BehaviorRunner<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a runner with the provided initial behavior.
  pub(crate) fn new(initial: Behavior<M>) -> Self {
    let supervisor = initial.supervisor_override().cloned();
    Self { current: initial, supervisor, stopping: false }
  }

  fn update_supervisor_override(&mut self, strategy: Option<SupervisorStrategyConfig>) {
    if let Some(strategy) = strategy {
      self.supervisor = Some(strategy);
    }
  }

  fn adapter_failure_event(ctx: &TypedActorContext<'_, M>, failure: &AdapterError) -> AdapterFailureEvent {
    match failure {
      | AdapterError::RegistryFull => AdapterFailureEvent::registry_full(ctx.pid()),
      | AdapterError::EnvelopeCorrupted => AdapterFailureEvent::envelope_corrupted(ctx.pid()),
      | AdapterError::ActorUnavailable => AdapterFailureEvent::actor_unavailable(ctx.pid()),
      | AdapterError::RegistryUnavailable => AdapterFailureEvent::registry_unavailable(ctx.pid()),
      | AdapterError::TypeMismatch(type_id) => AdapterFailureEvent::type_mismatch(ctx.pid(), *type_id),
      | AdapterError::Custom(detail) => AdapterFailureEvent::custom(ctx.pid(), detail.clone()),
    }
  }

  fn apply_transition(&mut self, ctx: &mut TypedActorContext<'_, M>, next: Behavior<M>) -> Result<(), ActorError> {
    let override_strategy = next.supervisor_override().cloned();

    match next.directive() {
      | BehaviorDirective::Same | BehaviorDirective::Ignore => Ok(()),
      | BehaviorDirective::Unhandled => {
        // Keep the current behavior and emit UnhandledMessage event
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = TypedUnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
        system.event_stream().publish(&EventStreamEvent::UnhandledMessage(event));
        Ok(())
      },
      | BehaviorDirective::Empty => {
        // Empty behavior: treat as unhandled and emit event, then keep empty behavior
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = TypedUnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
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

  /// Dispatches a signal and applies the resulting transition.
  ///
  /// Returns the directive of the behavior returned by the signal handler,
  /// allowing callers to determine whether the signal was actually handled.
  fn dispatch_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
  ) -> Result<BehaviorDirective, ActorError> {
    if let BehaviorSignal::MessageAdaptionFailure(failure) = signal {
      let event = Self::adapter_failure_event(ctx, failure);
      ctx.system().event_stream().publish(&EventStreamEvent::AdapterFailure(event));
    }
    let next = self.current.handle_signal(ctx, signal)?;
    let directive = next.directive();
    self.apply_transition(ctx, next)?;
    Ok(directive)
  }
}

impl<M> TypedActor<M> for BehaviorRunner<M>
where
  M: Send + Sync + 'static,
{
  fn pre_start(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::Started)?;
    Ok(())
  }

  fn receive(&mut self, ctx: &mut TypedActorContext<'_, M>, message: &M) -> Result<(), ActorError> {
    let next = self.current.handle_message(ctx, message)?;
    self.apply_transition(ctx, next)
  }

  fn post_stop(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::Stopped)?;
    Ok(())
  }

  fn on_terminated(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    terminated: crate::core::actor::Pid,
  ) -> Result<(), ActorError> {
    // シグナルハンドラが Terminated を実際に処理したかを判定する。
    // has_signal_handler() だけでは不十分 — ハンドラが Unhandled を返す場合も
    // DeathPactException を発行する必要がある (Pekko 互換)。
    let directive = self.dispatch_signal(ctx, &BehaviorSignal::Terminated(terminated))?;
    if matches!(directive, BehaviorDirective::Unhandled) || !self.current.has_signal_handler() {
      let ex = DeathPactException::new(terminated);
      Err(ActorError::recoverable_typed::<DeathPactException>(ex.to_string()))
    } else {
      Ok(())
    }
  }

  fn pre_restart(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::PreRestart)?;
    Ok(())
  }

  fn on_child_failed(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    child: crate::core::actor::Pid,
    error: &ActorError,
  ) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, &BehaviorSignal::ChildFailed { pid: child, error: error.clone() })?;
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &TypedActorContext<'_, M>) -> SupervisorStrategyConfig {
    self.supervisor.clone().unwrap_or_default()
  }

  fn on_adapter_failure(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    failure: AdapterError,
  ) -> Result<(), ActorError> {
    let directive = self.dispatch_signal(ctx, &BehaviorSignal::MessageAdaptionFailure(failure))?;
    if matches!(directive, BehaviorDirective::Unhandled) || !self.current.has_signal_handler() {
      Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
    } else {
      Ok(())
    }
  }
}
