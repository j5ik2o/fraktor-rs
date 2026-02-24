//! Adapts typed actors to the untyped runtime.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  dead_letter::DeadLetterReason,
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
  error::{ActorError, ActorErrorReason},
  event::logging::LogLevel,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  scheduler::SchedulerCommand,
  supervision::SupervisorStrategy,
  typed::{
    actor::{TypedActor, TypedActorContextGeneric},
    message_adapter::{
      AdaptMessage, AdapterEnvelope, AdapterError, AdapterOutcome, AdapterPayload, MessageAdapterRegistry,
    },
    receive_timeout_config::ReceiveTimeoutConfig,
  },
};

const DOWNCAST_FAILED: &str = "typed actor received unexpected message";

/// Wraps a typed actor and exposes the untyped [`Actor`] interface.
pub(crate) struct TypedActorAdapter<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  actor:           Box<dyn TypedActor<M, TB>>,
  adapters:        MessageAdapterRegistry<M, TB>,
  receive_timeout: Option<ReceiveTimeoutConfig<M, TB>>,
}

impl<M, TB> TypedActorAdapter<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new adapter from the provided typed actor.
  #[must_use]
  pub(crate) fn new<A>(actor: A) -> Self
  where
    A: TypedActor<M, TB> + 'static, {
    Self { actor: Box::new(actor), adapters: MessageAdapterRegistry::new(), receive_timeout: None }
  }

  fn handle_adapter_envelope(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    envelope: &AdapterEnvelope<TB>,
  ) -> Result<(), ActorError> {
    let sender = envelope.sender().cloned();
    let Some(payload) = envelope.take_payload() else {
      ctx.system().emit_log(LogLevel::Warn, "adapter envelope missing payload", Some(ctx.pid()));
      return Ok(());
    };
    if payload.type_id() != envelope.type_id() {
      Self::record_dead_letter(ctx, payload, sender.as_ref(), DeadLetterReason::ExplicitRouting);
      ctx.system().emit_log(LogLevel::Error, "adapter envelope corrupted", Some(ctx.pid()));
      return Ok(());
    }
    let (outcome, leftover) = self.adapters.adapt(payload);
    self.handle_adapter_outcome(ctx, outcome, sender.as_ref(), leftover)
  }

  fn handle_adapt_message(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: &AdaptMessage<M, TB>,
  ) -> Result<(), ActorError> {
    let outcome = message.execute();
    self.handle_adapter_outcome(ctx, outcome, None, None)
  }

  fn handle_adapter_outcome(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    outcome: AdapterOutcome<M>,
    sender: Option<&ActorRefGeneric<TB>>,
    original_payload: Option<AdapterPayload<TB>>,
  ) -> Result<(), ActorError> {
    match outcome {
      | AdapterOutcome::Converted(message) => self.deliver_converted_message(ctx, message, sender),
      | AdapterOutcome::Failure(failure) => self.forward_adapter_failure(ctx, failure),
      | AdapterOutcome::NotFound => {
        if let Some(payload) = original_payload {
          Self::record_dead_letter(ctx, payload, sender, DeadLetterReason::ExplicitRouting);
        }
        ctx.system().emit_log(LogLevel::Warn, "adapter dropped message", Some(ctx.pid()));
        Ok(())
      },
    }
  }

  fn deliver_converted_message(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: M,
    sender: Option<&ActorRefGeneric<TB>>,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    let mut current_message = AnyMessageGeneric::new(message);
    if let Some(target) = sender {
      typed_ctx.as_untyped_mut().set_sender(Some(target.clone()));
      current_message = current_message.with_sender(target.clone());
    }
    typed_ctx.as_untyped_mut().set_current_message(Some(current_message.clone()));
    let view = current_message.as_view();
    let payload =
      view.downcast_ref::<M>().ok_or_else(|| ActorError::recoverable(ActorErrorReason::new(DOWNCAST_FAILED)))?;
    let result = self.actor.receive(&mut typed_ctx, payload);
    if sender.is_some() {
      typed_ctx.as_untyped_mut().clear_sender();
    }
    result
  }

  fn forward_adapter_failure(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    failure: AdapterError,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.on_adapter_failure(&mut typed_ctx, failure)
  }

  fn make_typed_ctx<'c>(
    ctx: &mut ActorContextGeneric<'c, TB>,
    adapters: &mut MessageAdapterRegistry<M, TB>,
    receive_timeout: &mut Option<ReceiveTimeoutConfig<M, TB>>,
  ) -> TypedActorContextGeneric<'c, M, TB> {
    TypedActorContextGeneric::from_untyped(ctx, Some(adapters)).with_receive_timeout(receive_timeout)
  }

  fn reschedule_receive_timeout(&mut self, ctx: &ActorContextGeneric<'_, TB>) {
    if let Some(config) = &mut self.receive_timeout {
      Self::cancel_timer_handle(ctx, &mut config.handle);
      let self_ref = ctx.self_ref();
      let message = config.make_message();
      let duration = config.duration;
      let scheduler = ctx.system().scheduler();
      let result = scheduler.with_write(|guard| {
        guard.schedule_once(duration, SchedulerCommand::SendMessage {
          receiver:   self_ref,
          message:    AnyMessageGeneric::new(message),
          dispatcher: None,
          sender:     None,
        })
      });
      match result {
        | Ok(handle) => config.handle = Some(handle),
        | Err(e) => {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("failed to schedule receive timeout: {}", e),
            Some(ctx.pid()),
          );
        },
      }
    }
  }

  fn cancel_receive_timeout_timer(&mut self, ctx: &ActorContextGeneric<'_, TB>) {
    if let Some(config) = &mut self.receive_timeout {
      Self::cancel_timer_handle(ctx, &mut config.handle);
    }
  }

  fn cancel_timer_handle(
    ctx: &ActorContextGeneric<'_, TB>,
    handle: &mut Option<crate::core::scheduler::SchedulerHandle>,
  ) {
    if let Some(h) = handle.take() {
      let scheduler = ctx.system().scheduler();
      scheduler.with_write(|guard| {
        guard.cancel(&h);
      });
    }
  }

  fn record_dead_letter(
    ctx: &ActorContextGeneric<'_, TB>,
    payload: AdapterPayload<TB>,
    sender: Option<&ActorRefGeneric<TB>>,
    reason: DeadLetterReason,
  ) {
    let system_state = ctx.system().state();
    let message = AnyMessageGeneric::from_parts(payload.into_erased(), sender.cloned());
    system_state.record_dead_letter(message, reason, Some(ctx.pid()));
  }
}

impl<M, TB> Actor<TB> for TypedActorAdapter<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.pre_start(&mut typed_ctx)
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(envelope) = message.downcast_ref::<AdapterEnvelope<TB>>() {
      let result = self.handle_adapter_envelope(ctx, envelope);
      self.reschedule_receive_timeout(ctx);
      return result;
    }
    if let Some(adapt) = message.downcast_ref::<AdaptMessage<M, TB>>() {
      let result = self.handle_adapt_message(ctx, adapt);
      self.reschedule_receive_timeout(ctx);
      return result;
    }
    let payload =
      message.downcast_ref::<M>().ok_or_else(|| ActorError::recoverable(ActorErrorReason::new(DOWNCAST_FAILED)))?;
    {
      let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
      self.actor.receive(&mut typed_ctx, payload)?;
    }
    self.reschedule_receive_timeout(ctx);
    Ok(())
  }

  fn post_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.cancel_receive_timeout_timer(ctx);
    self.adapters.clear();
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.post_stop(&mut typed_ctx)
  }

  fn on_terminated(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    terminated: crate::core::actor::Pid,
  ) -> Result<(), ActorError> {
    self.adapters.clear();
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.on_terminated(&mut typed_ctx, terminated)
  }

  fn supervisor_strategy(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.supervisor_strategy(&mut typed_ctx)
  }

  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.on_mailbox_pressure(&mut typed_ctx, event)
  }
}
