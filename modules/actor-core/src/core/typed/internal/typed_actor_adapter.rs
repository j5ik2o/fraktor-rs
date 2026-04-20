//! Adapts typed actors to the untyped runtime.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedAccess;

use crate::core::{
  kernel::{
    actor::{
      Actor, ActorContext, Pid,
      actor_ref::{ActorRef, dead_letter::DeadLetterReason},
      error::{ActorError, ActorErrorReason},
      messaging::{AnyMessage, AnyMessageView},
      scheduler::{SchedulerCommand, SchedulerHandle},
      supervision::SupervisorStrategyConfig,
    },
    dispatch::mailbox::metrics_event::MailboxPressureEvent,
    event::logging::LogLevel,
  },
  typed::{
    actor::{TypedActor, TypedActorContext},
    internal::ReceiveTimeoutConfig,
    message_adapter::{
      AdaptMessage, AdapterEnvelope, AdapterError, AdapterOutcome, AdapterPayload, MessageAdapterRegistry,
    },
  },
};

const DOWNCAST_FAILED: &str = "typed actor received unexpected message";

/// Wraps a typed actor and exposes the untyped [`Actor`] interface.
pub(crate) struct TypedActorAdapter<M>
where
  M: Send + Sync + 'static, {
  actor:           Box<dyn TypedActor<M>>,
  adapters:        MessageAdapterRegistry<M>,
  receive_timeout: Option<ReceiveTimeoutConfig<M>>,
}

impl<M> TypedActorAdapter<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a new adapter from the provided typed actor.
  #[must_use]
  pub(crate) fn new<A>(actor: A) -> Self
  where
    A: TypedActor<M> + 'static, {
    Self { actor: Box::new(actor), adapters: MessageAdapterRegistry::new(), receive_timeout: None }
  }

  fn handle_adapter_envelope(
    &mut self,
    ctx: &mut ActorContext<'_>,
    envelope: &AdapterEnvelope,
    is_control: bool,
  ) -> Result<(), ActorError> {
    let sender = envelope.sender().cloned();
    let Some(payload) = envelope.take_payload() else {
      ctx.system().emit_log(LogLevel::Warn, "adapter envelope missing payload", Some(ctx.pid()), None);
      return Ok(());
    };
    if payload.type_id() != envelope.type_id() {
      Self::record_dead_letter(ctx, payload, sender.as_ref(), DeadLetterReason::ExplicitRouting, is_control);
      ctx.system().emit_log(LogLevel::Error, "adapter envelope corrupted", Some(ctx.pid()), None);
      return Ok(());
    }
    let (outcome, leftover) = self.adapters.adapt(payload);
    self.handle_adapter_outcome(ctx, outcome, sender.as_ref(), leftover, is_control)
  }

  fn handle_adapt_message(&mut self, ctx: &mut ActorContext<'_>, message: &AdaptMessage<M>) -> Result<(), ActorError> {
    let outcome = message.execute();
    // AdaptMessage はローカル生成のため control フラグは不要
    self.handle_adapter_outcome(ctx, outcome, None, None, false)
  }

  fn handle_adapter_outcome(
    &mut self,
    ctx: &mut ActorContext<'_>,
    outcome: AdapterOutcome<M>,
    sender: Option<&ActorRef>,
    original_payload: Option<AdapterPayload>,
    is_control: bool,
  ) -> Result<(), ActorError> {
    match outcome {
      | AdapterOutcome::Converted(message) => self.deliver_converted_message(ctx, message, sender),
      | AdapterOutcome::Failure(failure) => self.forward_adapter_failure(ctx, failure),
      | AdapterOutcome::NotFound => {
        if let Some(payload) = original_payload {
          Self::record_dead_letter(ctx, payload, sender, DeadLetterReason::ExplicitRouting, is_control);
        }
        ctx.system().emit_log(LogLevel::Warn, "adapter dropped message", Some(ctx.pid()), None);
        Ok(())
      },
    }
  }

  fn deliver_converted_message(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: M,
    sender: Option<&ActorRef>,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContext::from_untyped(ctx, Some(&mut self.adapters));
    let mut current_message = AnyMessage::new(message);
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

  fn forward_adapter_failure(&mut self, ctx: &mut ActorContext<'_>, failure: AdapterError) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContext::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.on_adapter_failure(&mut typed_ctx, failure)
  }

  fn make_typed_ctx<'c>(
    ctx: &mut ActorContext<'c>,
    adapters: &mut MessageAdapterRegistry<M>,
    receive_timeout: &mut Option<ReceiveTimeoutConfig<M>>,
  ) -> TypedActorContext<'c, M> {
    TypedActorContext::from_untyped(ctx, Some(adapters)).with_receive_timeout(receive_timeout)
  }

  fn reschedule_receive_timeout(&mut self, ctx: &ActorContext<'_>) {
    if let Some(config) = &mut self.receive_timeout {
      Self::cancel_timer_handle(ctx, &mut config.handle);
      let self_ref = ctx.self_ref();
      let message = config.make_message();
      let duration = config.duration;
      let scheduler = ctx.system().scheduler();
      let result = scheduler.with_write(|guard| {
        guard.schedule_once(duration, SchedulerCommand::SendMessage {
          receiver: self_ref,
          message:  AnyMessage::new(message),
          sender:   None,
        })
      });
      match result {
        | Ok(handle) => config.handle = Some(handle),
        | Err(e) => {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("failed to schedule receive timeout: {}", e),
            Some(ctx.pid()),
            None,
          );
        },
      }
    }
  }

  fn cancel_receive_timeout_timer(&mut self, ctx: &ActorContext<'_>) {
    if let Some(config) = &mut self.receive_timeout {
      Self::cancel_timer_handle(ctx, &mut config.handle);
    }
  }

  fn cancel_timer_handle(ctx: &ActorContext<'_>, handle: &mut Option<SchedulerHandle>) {
    if let Some(h) = handle.take() {
      let scheduler = ctx.system().scheduler();
      scheduler.with_write(|guard| {
        guard.cancel(&h);
      });
    }
  }

  fn record_dead_letter(
    ctx: &ActorContext<'_>,
    payload: AdapterPayload,
    sender: Option<&ActorRef>,
    reason: DeadLetterReason,
    is_control: bool,
  ) {
    let system_state = ctx.system().state();
    let message = AnyMessage::from_parts(payload.into_erased(), sender.cloned(), is_control);
    system_state.record_dead_letter(message, reason, Some(ctx.pid()));
  }
}

impl<M> Actor for TypedActorAdapter<M>
where
  M: Send + Sync + 'static,
{
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.pre_start(&mut typed_ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(envelope) = message.downcast_ref::<AdapterEnvelope>() {
      let result = self.handle_adapter_envelope(ctx, envelope, message.is_control());
      self.reschedule_receive_timeout(ctx);
      return result;
    }
    if let Some(adapt) = message.downcast_ref::<AdaptMessage<M>>() {
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

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.cancel_receive_timeout_timer(ctx);
    self.adapters.clear();
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.post_stop(&mut typed_ctx)
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.adapters.clear();
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.on_terminated(&mut typed_ctx, terminated)
  }

  fn supervisor_strategy(&self, ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    let typed_ctx = TypedActorContext::from_untyped(ctx, None);
    self.actor.supervisor_strategy(&typed_ctx)
  }

  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut ActorContext<'_>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.on_mailbox_pressure(&mut typed_ctx, event)
  }

  fn pre_restart(&mut self, ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.adapters.clear();
    let mut typed_ctx = TypedActorContext::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.pre_restart(&mut typed_ctx)
  }

  fn post_restart(&mut self, ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    let mut typed_ctx = Self::make_typed_ctx(ctx, &mut self.adapters, &mut self.receive_timeout);
    self.actor.post_restart(&mut typed_ctx)
  }

  fn on_child_failed(&mut self, ctx: &mut ActorContext<'_>, child: Pid, error: &ActorError) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContext::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.on_child_failed(&mut typed_ctx, child, error)
  }
}
