//! Pekko-inspired spawn protocol for typed actors.

#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
};

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
      return SpawnAnonymousCommand { props: self.props.clone(), reply_to: self.reply_to.clone() }.execute(ctx);
    }

    let child_name = next_child_name(ctx, &self.name);
    let child = ctx
      .spawn_child(&self.props.clone().map_props(|current| current.with_name(child_name)))
      .map_err(|error| ActorError::recoverable(format!("spawn failed: {error:?}")))?;
    let mut reply_to = self.reply_to.clone();
    reply_to.tell(child.actor_ref()).map_err(|error| ActorError::from_send_error(&error))?;
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
    let child =
      ctx.spawn_child(&self.props).map_err(|error| ActorError::recoverable(format!("spawn failed: {error:?}")))?;
    let mut reply_to = self.reply_to.clone();
    reply_to.tell(child.actor_ref()).map_err(|error| ActorError::from_send_error(&error))?;
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
    Behaviors::receive(move |ctx, command: &Self| {
      command.command.execute(ctx)?;
      Ok(Behaviors::same())
    })
  }
}

fn next_child_name(ctx: &TypedActorContext<'_, SpawnProtocol>, base_name: &str) -> String {
  if ctx.child(base_name).is_none() {
    return base_name.to_string();
  }

  let mut suffix = 1usize;
  loop {
    let candidate = format!("{base_name}-{suffix}");
    if ctx.child(candidate.as_str()).is_none() {
      return candidate;
    }
    suffix += 1;
  }
}
