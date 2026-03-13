//! System guardian responsible for `/system` supervision and termination hooks.

use alloc::vec::Vec;

use crate::core::{
  actor::{Actor, ActorContext, Pid, actor_ref::ActorRef},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind},
  system::guardian::SystemGuardianProtocol,
};

struct HookEntry {
  actor:     ActorRef,
  completed: bool,
}

impl HookEntry {
  #[allow(clippy::missing_const_for_fn)]
  fn pid(&self) -> Pid {
    self.actor.pid()
  }
}

enum SystemGuardianPhase {
  Running,
  Terminating,
  Stopped,
}

/// Actor managing `/system` termination hooks.
pub(crate) struct SystemGuardianActor {
  user_guardian: ActorRef,
  hooks:         Vec<HookEntry>,
  phase:         SystemGuardianPhase,
}

impl SystemGuardianActor {
  /// Creates a new system guardian linked to the provided user guardian.
  #[must_use]
  pub(crate) const fn new(user_guardian: ActorRef) -> Self {
    Self { user_guardian, hooks: Vec::new(), phase: SystemGuardianPhase::Running }
  }

  fn handle_protocol(
    &mut self,
    ctx: &mut ActorContext<'_>,
    protocol: &SystemGuardianProtocol,
  ) -> Result<(), ActorError> {
    match protocol {
      | SystemGuardianProtocol::RegisterTerminationHook(actor) => self.register_hook(ctx, actor.clone()),
      | SystemGuardianProtocol::TerminationHookDone(actor) => {
        self.mark_hook_done(actor.pid());
        self.try_complete(ctx)
      },
      | SystemGuardianProtocol::TerminationHook => Ok(()),
      | SystemGuardianProtocol::ForceTerminateHooks => {
        for hook in &mut self.hooks {
          hook.completed = true;
        }
        self.try_complete(ctx)
      },
    }
  }

  fn register_hook(&mut self, ctx: &mut ActorContext<'_>, actor: ActorRef) -> Result<(), ActorError> {
    if matches!(self.phase, SystemGuardianPhase::Terminating | SystemGuardianPhase::Stopped) {
      return Ok(());
    }
    if self.hooks.iter().any(|entry| entry.pid() == actor.pid()) {
      return Ok(());
    }
    ctx.watch(&actor).map_err(|error| ActorError::from_send_error(&error))?;
    self.hooks.push(HookEntry { actor, completed: false });
    Ok(())
  }

  fn start_termination(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    match self.phase {
      | SystemGuardianPhase::Running => {
        self.phase = SystemGuardianPhase::Terminating;
        if self.hooks.is_empty() {
          ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))
        } else {
          for hook in &mut self.hooks {
            hook
              .actor
              .tell(AnyMessage::new(SystemGuardianProtocol::TerminationHook))
              .map_err(|error| ActorError::from_send_error(&error))?;
          }
          Ok(())
        }
      },
      | _ => Ok(()),
    }
  }

  fn mark_hook_done(&mut self, pid: Pid) {
    if let Some(entry) = self.hooks.iter_mut().find(|entry| entry.pid() == pid) {
      entry.completed = true;
    }
  }

  fn try_complete(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if !matches!(self.phase, SystemGuardianPhase::Terminating) {
      return Ok(());
    }
    if self.hooks.iter().all(|hook| hook.completed) {
      self.phase = SystemGuardianPhase::Stopped;
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))
    } else {
      Ok(())
    }
  }
}

impl Actor for SystemGuardianActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.watch(&self.user_guardian).map_err(|error| ActorError::from_send_error(&error))
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(protocol) = message.downcast_ref::<SystemGuardianProtocol>() {
      self.handle_protocol(ctx, protocol)
    } else {
      Ok(())
    }
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    if terminated == self.user_guardian.pid() {
      self.start_termination(ctx)?;
    } else {
      self.mark_hook_done(terminated);
      self.try_complete(ctx)?;
    }
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 0, core::time::Duration::from_secs(0), |_error| {
      SupervisorDirective::Stop
    })
    .into()
  }
}
