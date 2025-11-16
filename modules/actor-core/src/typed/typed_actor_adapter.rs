//! Adapts typed actors to the untyped runtime.

use alloc::boxed::Box;

use crate::{
  RuntimeToolbox,
  actor_prim::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  dead_letter::DeadLetterReason,
  error::{ActorError, ActorErrorReason},
  logging::LogLevel,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  supervision::SupervisorStrategy,
  typed::{
    actor_prim::{TypedActor, TypedActorContextGeneric},
    message_adapter::{
      AdaptMessage, AdapterEnvelope, AdapterFailure, AdapterOutcome, AdapterPayload, MessageAdapterRegistry,
    },
  },
};

const DOWNCAST_FAILED: &str = "typed actor received unexpected message";

/// Wraps a typed actor and exposes the untyped [`Actor`] interface.
pub(crate) struct TypedActorAdapter<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  actor:    Box<dyn TypedActor<M, TB>>,
  adapters: MessageAdapterRegistry<M, TB>,
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
    Self { actor: Box::new(actor), adapters: MessageAdapterRegistry::new() }
  }

  fn handle_adapter_envelope(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    envelope: &AdapterEnvelope<TB>,
  ) -> Result<(), ActorError> {
    let reply_to = envelope.reply_to().cloned();
    let Some(payload) = envelope.take_payload() else {
      ctx.system().emit_log(LogLevel::Warn, "adapter envelope missing payload", Some(ctx.pid()));
      return Ok(());
    };
    if payload.type_id() != envelope.type_id() {
      Self::record_dead_letter(ctx, payload, reply_to.as_ref(), DeadLetterReason::ExplicitRouting);
      ctx.system().emit_log(LogLevel::Error, "adapter envelope corrupted", Some(ctx.pid()));
      return Ok(());
    }
    let (outcome, leftover) = self.adapters.adapt(payload);
    self.handle_adapter_outcome(ctx, outcome, reply_to.as_ref(), leftover)
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
    reply_to: Option<&ActorRefGeneric<TB>>,
    original_payload: Option<AdapterPayload<TB>>,
  ) -> Result<(), ActorError> {
    match outcome {
      | AdapterOutcome::Converted(message) => self.deliver_converted_message(ctx, message, reply_to),
      | AdapterOutcome::Failure(failure) => self.forward_adapter_failure(ctx, failure),
      | AdapterOutcome::NotFound => {
        if let Some(payload) = original_payload {
          Self::record_dead_letter(ctx, payload, reply_to, DeadLetterReason::ExplicitRouting);
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
    reply_to: Option<&ActorRefGeneric<TB>>,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    if let Some(target) = reply_to {
      typed_ctx.as_untyped_mut().set_reply_to(Some(target.clone()));
    }
    let result = self.actor.receive(&mut typed_ctx, &message);
    if reply_to.is_some() {
      typed_ctx.as_untyped_mut().clear_reply_to();
    }
    result
  }

  fn forward_adapter_failure(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    failure: AdapterFailure,
  ) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.on_adapter_failure(&mut typed_ctx, failure)
  }

  fn record_dead_letter(
    ctx: &ActorContextGeneric<'_, TB>,
    payload: AdapterPayload<TB>,
    reply_to: Option<&ActorRefGeneric<TB>>,
    reason: DeadLetterReason,
  ) {
    let system_state = ctx.system().state();
    let message = AnyMessageGeneric::from_parts(payload.into_erased(), reply_to.cloned());
    system_state.record_dead_letter(message, reason, Some(ctx.pid()));
  }
}

impl<M, TB> Actor<TB> for TypedActorAdapter<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.pre_start(&mut typed_ctx)
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(envelope) = message.downcast_ref::<AdapterEnvelope<TB>>() {
      return self.handle_adapter_envelope(ctx, envelope);
    }
    if let Some(adapt) = message.downcast_ref::<AdaptMessage<M, TB>>() {
      return self.handle_adapt_message(ctx, adapt);
    }
    let payload =
      message.downcast_ref::<M>().ok_or_else(|| ActorError::recoverable(ActorErrorReason::new(DOWNCAST_FAILED)))?;
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.receive(&mut typed_ctx, payload)
  }

  fn post_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.adapters.clear();
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.post_stop(&mut typed_ctx)
  }

  fn on_terminated(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    terminated: crate::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    self.adapters.clear();
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.on_terminated(&mut typed_ctx, terminated)
  }

  fn supervisor_strategy(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
    let mut typed_ctx = TypedActorContextGeneric::from_untyped(ctx, Some(&mut self.adapters));
    self.actor.supervisor_strategy(&mut typed_ctx)
  }
}
