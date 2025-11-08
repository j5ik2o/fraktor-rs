//! Core typed behavior abstraction.

use alloc::boxed::Box;

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  error::ActorError,
  supervision::SupervisorStrategy,
  typed::{actor_prim::TypedActorContextGeneric, behavior_signal::BehaviorSignal},
};

/// Captures message and signal handlers that can evolve into new behaviors after each invocation.
pub struct Behavior<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  directive:           BehaviorDirective,
  message_handler:     Option<MessageHandler<M, TB>>,
  signal_handler:      Option<SignalHandler<M, TB>>,
  supervisor_override: Option<SupervisorStrategy>,
}

/// Represents interpreter directives returned by behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BehaviorDirective {
  /// Indicates that the runtime should reuse the previous behavior instance.
  Same,
  /// Indicates that the actor should initiate a graceful stop.
  Stopped,
  /// Indicates that the behavior should remain active but silently drop messages.
  Ignore,
  /// Indicates that the provided handlers should be used as the new behavior.
  Active,
  /// Indicates that the message was not handled. Runtime will reuse previous behavior
  /// and emit an UnhandledMessage event for monitoring/debugging.
  Unhandled,
  /// Indicates that no more messages are expected but the actor has not stopped.
  /// All messages are treated as unhandled and logged.
  Empty,
}

type MessageHandler<M, TB> = Box<
  dyn for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &M) -> Result<Behavior<M, TB>, ActorError> + Send + Sync,
>;

type SignalHandler<M, TB> = Box<
  dyn for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &BehaviorSignal) -> Result<Behavior<M, TB>, ActorError>
    + Send
    + Sync,
>;

impl<M, TB> Behavior<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) const fn same() -> Self {
    Self {
      directive:           BehaviorDirective::Same,
      message_handler:     None,
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) const fn stopped() -> Self {
    Self {
      directive:           BehaviorDirective::Stopped,
      message_handler:     None,
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) const fn ignore() -> Self {
    Self {
      directive:           BehaviorDirective::Ignore,
      message_handler:     None,
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) const fn unhandled() -> Self {
    Self {
      directive:           BehaviorDirective::Unhandled,
      message_handler:     None,
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) const fn empty() -> Self {
    Self {
      directive:           BehaviorDirective::Empty,
      message_handler:     None,
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) fn from_message_handler<F>(handler: F) -> Self
  where
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &M) -> Result<Behavior<M, TB>, ActorError>
      + Send
      + Sync
      + 'static, {
    Self {
      directive:           BehaviorDirective::Active,
      message_handler:     Some(Box::new(handler)),
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) fn from_signal_handler<F>(handler: F) -> Self
  where
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &BehaviorSignal) -> Result<Behavior<M, TB>, ActorError>
      + Send
      + Sync
      + 'static, {
    Self {
      directive:           BehaviorDirective::Active,
      message_handler:     None,
      signal_handler:      Some(Box::new(handler)),
      supervisor_override: None,
    }
  }

  /// Attaches an additional signal handler while keeping the existing message handler intact.
  pub fn receive_signal<F>(mut self, handler: F) -> Self
  where
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &BehaviorSignal) -> Result<Behavior<M, TB>, ActorError>
      + Send
      + Sync
      + 'static, {
    self.signal_handler = Some(Box::new(handler));
    if matches!(self.directive, BehaviorDirective::Same) {
      self.directive = BehaviorDirective::Active;
    }
    self
  }

  /// Overrides the supervisor strategy associated with this behavior.
  #[must_use]
  pub const fn with_supervisor_strategy(mut self, strategy: SupervisorStrategy) -> Self {
    self.supervisor_override = Some(strategy);
    self
  }

  pub(crate) const fn supervisor_override(&self) -> Option<&SupervisorStrategy> {
    self.supervisor_override.as_ref()
  }

  pub(crate) fn handle_message(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    message: &M,
  ) -> Result<Behavior<M, TB>, ActorError> {
    match self.directive {
      | BehaviorDirective::Same => Ok(Self::same()),
      | BehaviorDirective::Stopped => Ok(Self::stopped()),
      | BehaviorDirective::Ignore => Ok(Self::same()),
      | BehaviorDirective::Unhandled => Ok(Self::unhandled()),
      | BehaviorDirective::Empty => Ok(Self::empty()),
      | BehaviorDirective::Active => match &mut self.message_handler {
        | Some(handler) => handler(ctx, message),
        | None => Ok(Self::same()),
      },
    }
  }

  pub(crate) fn handle_signal(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    signal: &BehaviorSignal,
  ) -> Result<Behavior<M, TB>, ActorError> {
    match self.directive {
      | BehaviorDirective::Same => Ok(Self::same()),
      | BehaviorDirective::Stopped => Ok(Self::stopped()),
      | BehaviorDirective::Ignore => Ok(Self::same()),
      | BehaviorDirective::Unhandled => Ok(Self::unhandled()),
      | BehaviorDirective::Empty => Ok(Self::empty()),
      | BehaviorDirective::Active => match &mut self.signal_handler {
        | Some(handler) => handler(ctx, signal),
        | None => Ok(Self::same()),
      },
    }
  }

  pub(crate) const fn directive(&self) -> BehaviorDirective {
    self.directive
  }

  pub(crate) const fn has_signal_handler(&self) -> bool {
    self.signal_handler.is_some()
  }
}
