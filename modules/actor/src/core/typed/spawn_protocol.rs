//! Pekko-inspired spawn protocol for typed actors.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  error::ActorError,
  typed::{
    Behaviors,
    actor::{TypedActorContext, TypedActorRef},
    behavior::Behavior,
    props::TypedProps,
  },
};

trait SpawnProtocolCommand: Send + Sync {
  fn execute(&self, ctx: &TypedActorContext<'_, SpawnProtocol>) -> Result<(), ActorError>;
}

struct SpawnNamedCommand<M>
where
  M: Send + Sync + 'static, {
  props:    TypedProps<M>,
  name:     String,
  reply_to: TypedActorRef<TypedActorRef<M>>,
}

impl<M> SpawnProtocolCommand for SpawnNamedCommand<M>
where
  M: Send + Sync + 'static,
{
  fn execute(&self, ctx: &TypedActorContext<'_, SpawnProtocol>) -> Result<(), ActorError> {
    if self.name.is_empty() {
      return Err(ActorError::recoverable("spawn name must not be empty"));
    }
    let child = ctx
      .spawn_child(&self.props.clone().map_props(|current| current.with_name(self.name.clone())))
      .map_err(|error| ActorError::recoverable(format!("spawn failed: {error:?}")))?;
    let mut reply_to = self.reply_to.clone();
    reply_to.tell(child.actor_ref());
    Ok(())
  }
}

struct SpawnAnonymousCommand<M>
where
  M: Send + Sync + 'static, {
  props:    TypedProps<M>,
  reply_to: TypedActorRef<TypedActorRef<M>>,
}

impl<M> SpawnProtocolCommand for SpawnAnonymousCommand<M>
where
  M: Send + Sync + 'static,
{
  fn execute(&self, ctx: &TypedActorContext<'_, SpawnProtocol>) -> Result<(), ActorError> {
    let anonymous_props = self.props.clone().map_props(|p| p.without_name());
    let child =
      ctx.spawn_child(&anonymous_props).map_err(|error| ActorError::recoverable(format!("spawn failed: {error:?}")))?;
    let mut reply_to = self.reply_to.clone();
    reply_to.tell(child.actor_ref());
    Ok(())
  }
}

/// Command protocol for spawning typed child actors through another actor.
pub struct SpawnProtocol {
  command: ArcShared<dyn SpawnProtocolCommand>,
}

impl Clone for SpawnProtocol {
  fn clone(&self) -> Self {
    Self { command: self.command.clone() }
  }
}

impl SpawnProtocol {
  /// Creates a named spawn command.
  #[must_use]
  pub fn spawn<M>(props: TypedProps<M>, name: impl Into<String>, reply_to: TypedActorRef<TypedActorRef<M>>) -> Self
  where
    M: Send + Sync + 'static, {
    Self { command: ArcShared::new(SpawnNamedCommand { props, name: name.into(), reply_to }) }
  }

  /// Creates an anonymous spawn command.
  #[must_use]
  pub fn spawn_anonymous<M>(props: TypedProps<M>, reply_to: TypedActorRef<TypedActorRef<M>>) -> Self
  where
    M: Send + Sync + 'static, {
    Self { command: ArcShared::new(SpawnAnonymousCommand { props, reply_to }) }
  }

  /// Builds the protocol behavior.
  #[must_use]
  pub fn behavior() -> Behavior<Self> {
    Behaviors::receive_message(move |ctx, command: &Self| {
      // Execution errors are non-fatal to keep this actor alive for subsequent requests.
      // On failure the requester's ask future remains pending until its timeout.
      if let Err(e) = command.command.execute(ctx) {
        ctx.system().emit_log(
          crate::core::event::logging::LogLevel::Warn,
          alloc::format!("spawn protocol command execution failed: {:?}", e),
          Some(ctx.pid()),
        );
      }
      Ok(Behaviors::same())
    })
  }
}
