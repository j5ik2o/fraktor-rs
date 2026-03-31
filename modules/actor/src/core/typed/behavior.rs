//! Core typed behavior abstraction.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  kernel::actor::{error::ActorError, supervision::SupervisorStrategyConfig},
  typed::{actor::TypedActorContext, behavior_signal::BehaviorSignal},
};

/// Captures message and signal handlers that can evolve into new behaviors after each invocation.
pub struct Behavior<M>
where
  M: Send + Sync + 'static, {
  directive:           BehaviorDirective,
  message_handler:     Option<MessageHandler<M>>,
  signal_handler:      Option<SignalHandler<M>>,
  supervisor_override: Option<SupervisorStrategyConfig>,
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

type MessageHandler<M> =
  Box<dyn for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync>;

type SignalHandler<M> =
  Box<dyn for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError> + Send + Sync>;

impl<M> Behavior<M>
where
  M: Send + Sync + 'static,
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
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    Self {
      directive:           BehaviorDirective::Active,
      message_handler:     Some(Box::new(handler)),
      signal_handler:      None,
      supervisor_override: None,
    }
  }

  pub(crate) fn from_signal_handler<F>(handler: F) -> Self
  where
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
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
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
      + Send
      + Sync
      + 'static, {
    self.signal_handler = Some(Box::new(handler));
    if matches!(self.directive, BehaviorDirective::Same) {
      self.directive = BehaviorDirective::Active;
    }
    self
  }

  /// Composes an additional signal handler that runs before the existing one.
  ///
  /// If the wrapper returns `Same`, the original handler (if any) is called.
  /// Otherwise the wrapper's result is returned directly.
  pub(crate) fn compose_signal<F>(mut self, wrapper: F) -> Self
  where
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
      + Send
      + Sync
      + 'static, {
    let existing = self.signal_handler.take();
    self.signal_handler = Some(Box::new(move |ctx, signal| {
      let result = wrapper(ctx, signal)?;
      if matches!(result.directive, BehaviorDirective::Same)
        && let Some(ref handler) = existing
      {
        return handler(ctx, signal);
      }
      Ok(result)
    }));
    if matches!(self.directive, BehaviorDirective::Same) {
      self.directive = BehaviorDirective::Active;
    }
    self
  }

  /// Overrides the supervisor strategy associated with this behavior.
  #[must_use]
  pub fn with_supervisor_strategy(mut self, strategy: impl Into<SupervisorStrategyConfig>) -> Self {
    self.supervisor_override = Some(strategy.into());
    self
  }

  pub(crate) const fn supervisor_override(&self) -> Option<&SupervisorStrategyConfig> {
    self.supervisor_override.as_ref()
  }

  pub(crate) fn handle_message(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
  ) -> Result<Behavior<M>, ActorError> {
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
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
  ) -> Result<Behavior<M>, ActorError> {
    match self.directive {
      | BehaviorDirective::Same => Ok(Self::same()),
      | BehaviorDirective::Stopped => Ok(Self::stopped()),
      | BehaviorDirective::Ignore => Ok(Self::same()),
      | BehaviorDirective::Unhandled => Ok(Self::unhandled()),
      | BehaviorDirective::Empty => Ok(Self::same()),
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

  /// Wraps this behavior to accept a different outer message type.
  ///
  /// The `mapper` converts each incoming `Outer` message to `Option<M>`.
  /// When the mapper returns `Some(inner)`, the inner message is forwarded
  /// to the wrapped behavior. When it returns `None`, the message is treated
  /// as unhandled — equivalent to Pekko's `PartialFunction` not being defined
  /// at a given input.
  ///
  /// Signals are forwarded directly to the inner behavior without
  /// transformation, matching Pekko's `transformMessages` semantics.
  pub fn transform_messages<Outer, F>(self, mapper: F) -> Behavior<Outer>
  where
    Outer: Send + Sync + 'static,
    F: Fn(&Outer) -> Option<M> + Send + Sync + 'static, {
    let supervisor_override = self.supervisor_override.clone();
    let inner_slot = ArcShared::new(RuntimeMutex::new(Some(self)));
    let mapper = ArcShared::new(mapper);

    Behavior::<Outer>::from_signal_handler({
      move |ctx, signal| match signal {
        | BehaviorSignal::Started => {
          let mut inner = inner_slot
            .lock()
            .take()
            .ok_or_else(|| ActorError::fatal("transform_messages: inner behavior already consumed"))?;

          {
            let mut inner_ctx = TypedActorContext::<M>::from_untyped(ctx.as_untyped_mut(), None);
            let started_result = inner.handle_signal(&mut inner_ctx, &BehaviorSignal::Started)?;
            match started_result.directive() {
              | BehaviorDirective::Active => inner = started_result,
              | BehaviorDirective::Stopped => return Ok(Behavior::stopped()),
              | BehaviorDirective::Empty => inner = Behavior::empty(),
              | _ => {},
            }
          }

          let state = ArcShared::new(RuntimeMutex::new(inner));
          let state_msg = state.clone();
          let state_sig = state;
          let mapper_msg = mapper.clone();

          let mut outer = Behavior::<Outer>::from_message_handler(move |ctx, msg: &Outer| match mapper_msg(msg) {
            | Some(inner_msg) => {
              let mut guard = state_msg.lock();
              let mut inner_ctx = TypedActorContext::<M>::from_untyped(ctx.as_untyped_mut(), None);
              let next = guard.handle_message(&mut inner_ctx, &inner_msg)?;
              Ok(resolve_transform_directive(&mut guard, next))
            },
            | None => Ok(Behavior::unhandled()),
          })
          .receive_signal(move |ctx, signal| {
            let mut guard = state_sig.lock();
            let mut inner_ctx = TypedActorContext::<M>::from_untyped(ctx.as_untyped_mut(), None);
            let next = guard.handle_signal(&mut inner_ctx, signal)?;
            Ok(resolve_transform_directive(&mut guard, next))
          });
          // 内部 Behavior の supervisor_override を外部 Behavior に引き継ぐ
          if let Some(strategy) = &supervisor_override {
            outer = outer.with_supervisor_strategy(strategy.clone());
          }
          Ok(outer)
        },
        | _ => Ok(Behavior::same()),
      }
    })
  }

  /// Narrows the message type of this behavior via `Into` conversion.
  ///
  /// This is the Rust equivalent of Pekko's `Behavior.narrow`. Since Rust
  /// lacks subtype polymorphism, the caller must provide an explicit `Into`
  /// conversion from the outer type `U` to the inner type `M`.
  #[must_use]
  pub fn narrow<U>(self) -> Behavior<U>
  where
    U: Clone + Into<M> + Send + Sync + 'static, {
    self.transform_messages(|outer: &U| Some(outer.clone().into()))
  }
}

/// Maps an inner behavior directive into an outer behavior sentinel.
///
/// When the inner behavior evolves (`Active`), the shared state is updated
/// and the outer behavior returns `Same` to keep the wrapper alive.
fn resolve_transform_directive<Inner, Outer>(inner: &mut Behavior<Inner>, next: Behavior<Inner>) -> Behavior<Outer>
where
  Inner: Send + Sync + 'static,
  Outer: Send + Sync + 'static, {
  match next.directive() {
    | BehaviorDirective::Same | BehaviorDirective::Ignore => Behavior::same(),
    | BehaviorDirective::Stopped => Behavior::stopped(),
    | BehaviorDirective::Unhandled => Behavior::unhandled(),
    | BehaviorDirective::Empty => {
      *inner = Behavior::empty();
      Behavior::same()
    },
    | BehaviorDirective::Active => {
      *inner = next;
      Behavior::same()
    },
  }
}
