extern crate std;
use std::ops::{Deref, DerefMut};

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::{
    actor_prim::Pid,
    error::SendError,
    spawn::SpawnError,
    typed::{TypedActorSystemGeneric, actor_prim::TypedActorContextGeneric as CoreTypedActorContextGeneric},
  },
  std::typed::{
    TypedProps,
    actor_prim::{TypedActorRef, TypedChildRef},
  },
};

/// Typed actor context wrapper for StdToolbox.
#[repr(transparent)]
pub struct TypedActorContext<'a, M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedActorContextGeneric<'a, M, StdToolbox>,
}

impl<'a, M> TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Returns the underlying actor system handle.
  #[must_use]
  pub fn system(&self) -> TypedActorSystemGeneric<M, StdToolbox> {
    self.inner.system()
  }

  /// Returns the typed self reference.
  #[must_use]
  pub fn self_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_core(self.inner.self_ref())
  }

  /// Sends a reply to the original sender.
  ///
  /// # Errors
  ///
  /// Returns an error if the sender is unavailable or the message cannot be delivered.
  pub fn reply<R>(&self, message: R) -> Result<(), SendError<StdToolbox>>
  where
    R: Send + Sync + 'static, {
    self.inner.reply(message)
  }

  /// Spawns a typed child actor using the provided typed props
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_child<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner.spawn_child(typed_props.as_core())?;
    Ok(TypedChildRef::from_core(child))
  }

  /// Spawns a typed child actor and automatically watches it.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned or watched.
  pub fn spawn_child_watched<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner.spawn_child_watched(typed_props.as_core())?;
    Ok(TypedChildRef::from_core(child))
  }

  /// Watches the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError<StdToolbox>>
  where
    C: Send + Sync + 'static, {
    self.inner.watch(target.as_core())
  }

  /// Stops watching the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the unwatch operation cannot be performed.
  pub fn unwatch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError<StdToolbox>>
  where
    C: Send + Sync + 'static, {
    self.inner.unwatch(target.as_core())
  }

  /// Stops the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_self(&self) -> Result<(), SendError<StdToolbox>> {
    self.inner.stop_self()
  }

  /// Provides access to the underlying core typed context.
  #[must_use]
  pub const fn as_core(&self) -> &CoreTypedActorContextGeneric<'a, M, StdToolbox> {
    &self.inner
  }

  /// Provides mutable access to the underlying core typed context.
  pub const fn as_core_mut(&mut self) -> &mut CoreTypedActorContextGeneric<'a, M, StdToolbox> {
    &mut self.inner
  }
}

impl<'a, M> Deref for TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  type Target = CoreTypedActorContextGeneric<'a, M, StdToolbox>;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<M> DerefMut for TypedActorContext<'_, M>
where
  M: Send + Sync + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
