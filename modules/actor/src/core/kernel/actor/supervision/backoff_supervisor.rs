//! Factory for creating backoff supervisor actors.

use alloc::string::String;
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::kernel::{
  actor::{
    Actor, ActorContext, Pid,
    child_ref::ChildRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::SchedulerCommand,
    supervision::{
      BackoffOnFailureOptions, BackoffOnStopOptions, BackoffSupervisorCommand, BackoffSupervisorResponse,
      BackoffSupervisorStrategy, SupervisorStrategy, SupervisorStrategyConfig,
    },
  },
  event::logging::LogLevel,
};

#[cfg(test)]
mod tests;

/// Backoff mode distinguishing on-stop from on-failure supervision.
#[derive(Clone, Debug)]
enum BackoffMode {
  /// Restart on child termination.
  OnStop,
  /// Restart on child failure.
  OnFailure,
}

#[derive(Clone, Debug)]
enum BackoffSupervisorInternalCommand {
  StartChild,
  ResetRestartCount(u32),
}

/// Configuration bundle for constructing a [`BackoffSupervisorActor`].
#[derive(Clone)]
struct BackoffConfig {
  child_props:         Props,
  child_name:          String,
  strategy:            BackoffSupervisorStrategy,
  mode:                BackoffMode,
  auto_reset:          Option<Duration>,
  manual_reset:        bool,
  supervisor_strategy: Option<SupervisorStrategy>,
  max_retries:         u32,
}

/// Factory for creating backoff supervisor [`Props`].
///
/// Corresponds to Pekko's `BackoffSupervisor`. Use [`props_on_stop`](Self::props_on_stop)
/// or [`props_on_failure`](Self::props_on_failure) to obtain a [`Props`] that spawns
/// a backoff supervisor actor wrapping a child.
pub struct BackoffSupervisor;

impl BackoffSupervisor {
  /// Creates [`Props`] for a backoff supervisor that restarts its child on stop.
  ///
  /// Corresponds to Pekko's `BackoffSupervisor.props(BackoffOnStopOptions)`.
  #[must_use]
  #[allow(clippy::needless_pass_by_value)]
  pub fn props_on_stop(options: BackoffOnStopOptions) -> Props {
    let config = BackoffConfig::from_stop(options);
    Props::from_fn(move || BackoffSupervisorActor::from_config(config.clone()))
  }

  /// Creates [`Props`] for a backoff supervisor that restarts its child on failure.
  ///
  /// Corresponds to Pekko's `BackoffSupervisor.props(BackoffOnFailureOptions)`.
  #[must_use]
  #[allow(clippy::needless_pass_by_value)]
  pub fn props_on_failure(options: BackoffOnFailureOptions) -> Props {
    let config = BackoffConfig::from_failure(options);
    Props::from_fn(move || BackoffSupervisorActor::from_config(config.clone()))
  }
}

impl BackoffConfig {
  #[allow(clippy::needless_pass_by_value)]
  fn from_stop(options: BackoffOnStopOptions) -> Self {
    Self {
      child_props:         options.child_props().clone(),
      child_name:          String::from(options.child_name()),
      strategy:            options.strategy().clone(),
      mode:                BackoffMode::OnStop,
      auto_reset:          options.auto_reset(),
      manual_reset:        options.manual_reset(),
      supervisor_strategy: options.supervisor_strategy().cloned(),
      max_retries:         options.max_retries(),
    }
  }

  #[allow(clippy::needless_pass_by_value)]
  fn from_failure(options: BackoffOnFailureOptions) -> Self {
    Self {
      child_props:         options.child_props().clone(),
      child_name:          String::from(options.child_name()),
      strategy:            options.strategy().clone(),
      mode:                BackoffMode::OnFailure,
      auto_reset:          options.auto_reset(),
      manual_reset:        options.manual_reset(),
      supervisor_strategy: options.supervisor_strategy().cloned(),
      max_retries:         options.max_retries(),
    }
  }
}

/// Internal actor implementing the backoff supervision pattern.
///
/// Spawns a child actor, watches it, and handles protocol messages
/// (`GetCurrentChild`, `Reset`, `GetRestartCount`). Unrecognized messages
/// are forwarded to the child.
struct BackoffSupervisorActor {
  child:               Option<ChildRef>,
  restart_count:       u32,
  child_props:         Props,
  child_name:          String,
  strategy:            BackoffSupervisorStrategy,
  mode:                BackoffMode,
  auto_reset:          Option<Duration>,
  manual_reset:        bool,
  supervisor_strategy: Option<SupervisorStrategy>,
  supervisor_strategy_config: SupervisorStrategyConfig,
  max_retries:         u32,
  initialized:         bool,
  pending_restart:     bool,
}

impl BackoffSupervisorActor {
  fn build_supervisor_strategy_config(
    mode: &BackoffMode,
    supervisor_strategy: &Option<SupervisorStrategy>,
  ) -> SupervisorStrategyConfig {
    match mode {
      | BackoffMode::OnStop => {
        supervisor_strategy.clone().map_or_else(SupervisorStrategyConfig::default, Into::into)
      },
      | BackoffMode::OnFailure => {
        let base_strategy = supervisor_strategy.clone().unwrap_or_default();
        let decision_source = ArcShared::new(base_strategy.clone());
        base_strategy
          .with_dyn_decider(move |error| match decision_source.decide(error) {
            | crate::core::kernel::actor::supervision::SupervisorDirective::Restart => {
              crate::core::kernel::actor::supervision::SupervisorDirective::Stop
            },
            | other => other,
          })
          .into()
      },
    }
  }

  fn from_config(config: BackoffConfig) -> Self {
    let supervisor_strategy_config = Self::build_supervisor_strategy_config(&config.mode, &config.supervisor_strategy);
    Self {
      child:               None,
      restart_count:       0,
      child_props:         config.child_props,
      child_name:          config.child_name,
      strategy:            config.strategy,
      mode:                config.mode,
      auto_reset:          config.auto_reset,
      manual_reset:        config.manual_reset,
      supervisor_strategy: config.supervisor_strategy,
      supervisor_strategy_config,
      max_retries:         config.max_retries,
      initialized:         false,
      pending_restart:     false,
    }
  }

  /// Spawns the child actor and watches it.
  fn start_child(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let props = self.child_props.clone().with_name(&self.child_name);
    match ctx.spawn_child(&props) {
      | Ok(child) => {
        let child_ref = child.actor_ref().clone();
        if let Err(error) = ctx.watch(&child_ref) {
          if let Err(stop_error) = child.stop() {
            ctx.system().state().record_send_error(Some(child.pid()), &stop_error);
          }
          self.initialized = true;
          return Err(ActorError::recoverable(alloc::format!("failed to install death watch: {:?}", error)));
        }
        self.child = Some(child);
        self.initialized = true;
        Ok(())
      },
      | Err(error) => {
        self.initialized = true;
        Err(ActorError::recoverable(alloc::format!("failed to spawn child: {error:?}")))
      },
    }
  }

  fn schedule_internal_command(
    ctx: &mut ActorContext<'_>,
    delay: Duration,
    command: BackoffSupervisorInternalCommand,
  ) -> Result<(), ActorError> {
    let receiver =
      ctx.system().actor_ref(ctx.pid()).ok_or_else(|| ActorError::recoverable("backoff supervisor ref unavailable"))?;
    let scheduled =
      SchedulerCommand::SendMessage { receiver, message: AnyMessage::new(command), dispatcher: None, sender: None };
    ctx
      .system()
      .scheduler()
      .with_write(|scheduler| scheduler.schedule_once(delay, scheduled))
      .map(|_| ())
      .map_err(|error| ActorError::recoverable(alloc::format!("failed to schedule backoff command: {:?}", error)))
  }

  const fn backoff_iteration_for_restart_count(restart_count: u32) -> u32 {
    restart_count.saturating_sub(1)
  }

  fn schedule_start_child(&self, ctx: &mut ActorContext<'_>, next_restart_count: u32) -> Result<(), ActorError> {
    // Pekko calculates the delay using the current restart counter before incrementing it.
    // We store the incremented restart count first, so map "restart attempt N" to
    // "backoff iteration N-1" explicitly here.
    let backoff_iteration = Self::backoff_iteration_for_restart_count(next_restart_count);
    let delay = self.strategy.compute_backoff(backoff_iteration);
    Self::schedule_internal_command(ctx, delay, BackoffSupervisorInternalCommand::StartChild)
  }

  fn schedule_auto_reset(&self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let Some(auto_reset) = self.auto_reset else {
      return Ok(());
    };
    if self.manual_reset || self.restart_count == 0 {
      return Ok(());
    }
    Self::schedule_internal_command(
      ctx,
      auto_reset,
      BackoffSupervisorInternalCommand::ResetRestartCount(self.restart_count),
    )
  }

  fn handle_internal_command(
    &mut self,
    ctx: &mut ActorContext<'_>,
    command: &BackoffSupervisorInternalCommand,
  ) -> Result<(), ActorError> {
    match command {
      | BackoffSupervisorInternalCommand::StartChild => {
        if self.child.is_none() {
          self.start_child(ctx)?;
          self.schedule_auto_reset(ctx)?;
        }
        Ok(())
      },
      | BackoffSupervisorInternalCommand::ResetRestartCount(seen_restart_count) => {
        if *seen_restart_count == self.restart_count && self.restart_count > 0 {
          self.restart_count = 0;
        }
        Ok(())
      },
    }
  }

  /// Handles a backoff supervisor protocol command.
  fn handle_command(&mut self, ctx: &mut ActorContext<'_>, command: &BackoffSupervisorCommand) {
    match command {
      | BackoffSupervisorCommand::GetCurrentChild => {
        let pid = self.child.as_ref().map(ChildRef::pid);
        let response = AnyMessage::new(BackoffSupervisorResponse::CurrentChild(pid));
        if let Err(error) = ctx.reply(response) {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("BackoffSupervisor handle_command failed to reply to GetCurrentChild: {:?}", error),
            Some(ctx.pid()),
            None,
          );
        }
      },
      | BackoffSupervisorCommand::GetRestartCount => {
        let response = AnyMessage::new(BackoffSupervisorResponse::RestartCount(self.restart_count));
        if let Err(error) = ctx.reply(response) {
          ctx.system().emit_log(
            LogLevel::Warn,
            alloc::format!("BackoffSupervisor handle_command failed to reply to GetRestartCount: {:?}", error),
            Some(ctx.pid()),
            None,
          );
        }
      },
      | BackoffSupervisorCommand::Reset => {
        self.restart_count = 0;
      },
    }
  }
}

impl Actor for BackoffSupervisorActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.start_child(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    // Lazy initialization: if pre_start was not called (e.g. test setup), spawn the child now.
    if !self.initialized {
      self.start_child(ctx)?;
    }

    if let Some(command) = message.downcast_ref::<BackoffSupervisorInternalCommand>() {
      return self.handle_internal_command(ctx, command);
    }

    if let Some(command) = message.downcast_ref::<BackoffSupervisorCommand>() {
      self.handle_command(ctx, command);
      return Ok(());
    }

    // Forward unrecognized messages to child.
    if let Some(child) = self.child.as_ref()
      && let Some(msg) = ctx.clone_current_message()
    {
      let mut child_ref = child.actor_ref().clone();
      ctx.forward(&mut child_ref, msg);
    }
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    // Only react when our child terminates.
    let is_our_child = self.child.as_ref().is_some_and(|c| c.pid() == terminated);
    if !is_our_child {
      return Ok(());
    }

    self.child = None;
    match self.mode {
      | BackoffMode::OnStop => {
        let next_restart_count = self.restart_count + 1;
        if self.max_retries != 0 && next_restart_count > self.max_retries {
          return Ok(());
        }
        self.restart_count = next_restart_count;
        self.schedule_start_child(ctx, next_restart_count)
      },
      | BackoffMode::OnFailure => {
        if !self.pending_restart {
          return Ok(());
        }
        self.pending_restart = false;
        let next_restart_count = self.restart_count + 1;
        if self.max_retries != 0 && next_restart_count > self.max_retries {
          return Ok(());
        }
        self.restart_count = next_restart_count;
        self.schedule_start_child(ctx, next_restart_count)
      },
    }
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    self.supervisor_strategy_config.clone()
  }

  fn on_child_failed(&mut self, _ctx: &mut ActorContext<'_>, child: Pid, error: &ActorError) -> Result<(), ActorError> {
    if matches!(self.mode, BackoffMode::OnFailure) && self.child.as_ref().is_some_and(|current| current.pid() == child)
    {
      let directive = self
        .supervisor_strategy
        .as_ref()
        .map_or_else(|| SupervisorStrategy::default().decide(error), |strategy| strategy.decide(error));
      self.pending_restart = matches!(directive, crate::core::kernel::actor::supervision::SupervisorDirective::Restart);
    }
    Ok(())
  }
}
