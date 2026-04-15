//! Executes typed behaviors inside the actor runtime.

use alloc::string::ToString;
use core::convert::Infallible;

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRef, NullSender},
      error::{ActorError, ActorErrorReason},
      supervision::SupervisorStrategyConfig,
    },
    event::stream::{AdapterFailureEvent, EventStreamEvent, TypedUnhandledMessageEvent},
  },
  typed::{
    TypedActorRef,
    actor::{TypedActor, TypedActorContext},
    behavior::{Behavior, BehaviorDirective},
    message_adapter::AdapterError,
    message_and_signals::{BehaviorSignal, ChildFailed, DeathPactError, MessageAdaptionFailure, Terminated},
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
    let next = next.resolve_started_behavior(ctx)?;
    let override_strategy = next.supervisor_override().cloned();

    match next.directive() {
      | BehaviorDirective::Same | BehaviorDirective::Ignore => Ok(()),
      | BehaviorDirective::Unhandled => {
        // Keep the current behavior and emit UnhandledMessage event
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = TypedUnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
        system.publish_event(&EventStreamEvent::UnhandledMessage(event));
        Ok(())
      },
      | BehaviorDirective::Empty => {
        // Empty behavior: treat as unhandled and emit event, then keep empty behavior
        let system = ctx.system();
        let timestamp = system.state().monotonic_now();
        let message_type = core::any::type_name::<M>().to_string();
        let event = TypedUnhandledMessageEvent::new(ctx.pid(), message_type, timestamp);
        system.publish_event(&EventStreamEvent::UnhandledMessage(event));
        self.current = Behavior::empty();
        Ok(())
      },
      | BehaviorDirective::Stopped => {
        if !self.stopping {
          // stop_self 呼び出し前に next を保存してシグナルハンドラを保持する。
          // stop_self が失敗した場合は stopping を立てず、次回の呼び出しで再試行できるようにする。
          self.current = next;
          ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
          self.stopping = true;
        } else if next.has_signal_handler() {
          // 既に停止処理中だが、新たにシグナルハンドラが付与された場合は上書きして保持する。
          self.current = next;
        }
        Ok(())
      },
      | BehaviorDirective::Active => {
        self.current = next;
        Ok(())
      },
    }
    .map(|_| self.update_supervisor_override(override_strategy))
  }

  fn terminated_actor_ref(ctx: &TypedActorContext<'_, M>, pid: Pid) -> TypedActorRef<Infallible> {
    let system = ctx.system().as_untyped().clone();
    let actor_ref =
      system.actor_ref_by_pid(pid).unwrap_or_else(|| ActorRef::with_system(pid, NullSender, &system.state()));
    TypedActorRef::from_untyped(actor_ref)
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
      let event = Self::adapter_failure_event(ctx, failure.error());
      ctx.system().publish_event(&EventStreamEvent::AdapterFailure(event));
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
    if matches!(self.current.directive(), BehaviorDirective::Stopped) && !self.stopping {
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      self.stopping = true;
    }
    let next = self.current.handle_start(ctx)?;
    self.apply_transition(ctx, next)?;
    Ok(())
  }

  fn receive(&mut self, ctx: &mut TypedActorContext<'_, M>, message: &M) -> Result<(), ActorError> {
    let next = self.current.handle_message(ctx, message)?;
    self.apply_transition(ctx, next)
  }

  fn post_stop(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    // post_stop に到達した時点で停止は確定しているため、
    // PostStop シグナル処理中に stop_self を再送しないようにする。
    self.stopping = true;
    self.dispatch_signal(ctx, &BehaviorSignal::PostStop)?;
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut TypedActorContext<'_, M>, terminated: Pid) -> Result<(), ActorError> {
    // シグナルハンドラが Terminated を実際に処理したかを判定する。
    // has_signal_handler() だけでは不十分 — ハンドラが Unhandled を返す場合も
    // DeathPactError を発行する必要がある (Pekko 互換)。
    let signal = BehaviorSignal::from(Terminated::new(Self::terminated_actor_ref(ctx, terminated)));
    let directive = self.dispatch_signal(ctx, &signal)?;
    if matches!(directive, BehaviorDirective::Unhandled) || !self.current.has_signal_handler() {
      let ex = DeathPactError::new(terminated);
      Err(ActorError::recoverable_typed::<DeathPactError>(ex.to_string()))
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
    child: Pid,
    error: &ActorError,
  ) -> Result<(), ActorError> {
    let signal = BehaviorSignal::from(ChildFailed::new(Self::terminated_actor_ref(ctx, child), error.clone()));
    self.dispatch_signal(ctx, &signal)?;
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
    let signal = BehaviorSignal::from(MessageAdaptionFailure::new(failure));
    let directive = self.dispatch_signal(ctx, &signal)?;
    if matches!(directive, BehaviorDirective::Unhandled) || !self.current.has_signal_handler() {
      Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
    } else {
      Ok(())
    }
  }
}
