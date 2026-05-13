//! Delays typed user guardian startup until actor-system bootstrap completes.

use alloc::{boxed::Box, vec::Vec};
use core::mem;

use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_ref::ActorRef,
    error::{ActorError, ActorErrorReason},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    spawn::SpawnError,
    supervision::SupervisorStrategyConfig,
  },
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use super::GuardianStartupStart;

/// Pekko-compatible startup gate for the typed user guardian.
pub(crate) struct GuardianStartupActor {
  inner:    Box<dyn Actor + Send>,
  started:  bool,
  deferred: Vec<AnyMessage>,
}

impl GuardianStartupActor {
  pub(crate) fn props(user_guardian_props: &Props) -> Result<Props, SpawnError> {
    let Some(actor_factory) = user_guardian_props.factory().cloned() else {
      return Err(SpawnError::invalid_props("actor factory is required"));
    };
    let props = user_guardian_props.clone().with_factory(Box::new(move || {
      let inner = actor_factory.with_write(|factory| factory.create());
      Self::new(inner)
    }));
    Ok(props)
  }

  fn new(inner: Box<dyn Actor + Send>) -> Self {
    Self { inner, started: false, deferred: Vec::new() }
  }

  fn start_inner(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if self.started {
      return Ok(());
    }
    self.started = true;
    self.inner.pre_start(ctx)?;
    self.deliver_deferred(ctx)
  }

  fn deliver_deferred(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let deferred = mem::take(&mut self.deferred);
    for message in deferred {
      self.deliver_deferred_message(ctx, message)?;
    }
    Ok(())
  }

  fn deliver_deferred_message(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage) -> Result<(), ActorError> {
    let previous_sender = ctx.sender().cloned();
    let sender = message.sender().cloned();
    let not_influence_receive_timeout = message.is_not_influence_receive_timeout();
    Self::set_sender(ctx, sender);
    let result = ctx.with_current_message(message, |active_ctx, current_message| {
      self.inner.receive(active_ctx, current_message.as_view())
    });
    Self::set_sender(ctx, previous_sender);
    if result.is_ok() && !not_influence_receive_timeout {
      ctx.reschedule_receive_timeout();
    }
    result
  }

  fn defer_current_message(&mut self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    let Some(message) = ctx.clone_current_message() else {
      return Err(ActorError::recoverable("guardian startup deferral requires an active user message"));
    };
    self.deferred.push(message);
    Ok(())
  }

  fn set_sender(ctx: &mut ActorContext<'_>, sender: Option<ActorRef>) {
    match sender {
      | Some(sender) => ctx.set_sender(Some(sender)),
      | None => ctx.clear_sender(),
    }
  }
}

impl Actor for GuardianStartupActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<GuardianStartupStart>().is_some() {
      return self.start_inner(ctx);
    }
    if self.started {
      return self.inner.receive(ctx, message);
    }
    self.defer_current_message(ctx)
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.deferred.clear();
    if self.started {
      return self.inner.post_stop(ctx);
    }
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    if self.started {
      return self.inner.on_terminated(ctx, terminated);
    }
    Ok(())
  }

  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut ActorContext<'_>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    if self.started {
      return self.inner.on_mailbox_pressure(ctx, event);
    }
    Ok(())
  }

  fn supervisor_strategy(&self, ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    self.inner.supervisor_strategy(ctx)
  }

  fn pre_restart(&mut self, ctx: &mut ActorContext<'_>, reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.deferred.clear();
    if self.started {
      return self.inner.pre_restart(ctx, reason);
    }
    Ok(())
  }

  fn post_restart(&mut self, ctx: &mut ActorContext<'_>, reason: &ActorErrorReason) -> Result<(), ActorError> {
    self.started = true;
    self.inner.post_restart(ctx, reason)?;
    self.deliver_deferred(ctx)
  }

  fn on_child_failed(&mut self, ctx: &mut ActorContext<'_>, child: Pid, error: &ActorError) -> Result<(), ActorError> {
    if self.started {
      return self.inner.on_child_failed(ctx, child, error);
    }
    Ok(())
  }
}
