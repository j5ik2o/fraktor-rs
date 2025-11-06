use cellactor_actor_core_rs::typed::actor_prim::TypedActorContextGeneric as CoreTypedActorContextGeneric;
use cellactor_actor_core_rs::actor_prim::Pid;
use cellactor_actor_core_rs::error::SendError;
use cellactor_actor_core_rs::spawn::SpawnError;
use cellactor_actor_core_rs::system::ActorSystemGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
use crate::typed::actor_prim::{TypedActorRef, TypedChildRef};
use crate::typed::TypedProps;
use std::ops::{Deref, DerefMut};

/// Typed actor context wrapper for StdToolbox.
#[repr(transparent)]
pub struct TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  inner: CoreTypedActorContextGeneric<'a, M, StdToolbox>,
}

impl<'a, M> TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  /// Creates a typed context from the core typed context.
  pub(crate) fn from_core(inner: CoreTypedActorContextGeneric<'a, M, StdToolbox>) -> Self {
    Self { inner }
  }

  /// Converts a mutable reference to core context into a mutable reference to wrapped context.
  ///
  /// # Safety
  /// This is safe because TypedActorContext is a transparent wrapper around CoreTypedActorContextGeneric.
  pub(crate) fn from_core_ref_mut<'b>(inner: &'b mut CoreTypedActorContextGeneric<'b, M, StdToolbox>) -> &'b mut TypedActorContext<'b, M> {
    // SAFETY: TypedActorContext is #[repr(transparent)] over CoreTypedActorContextGeneric
    unsafe {
      &mut *(inner as *mut CoreTypedActorContextGeneric<'b, M, StdToolbox> as *mut TypedActorContext<'b, M>)
    }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Returns the underlying actor system handle.
  #[must_use]
  pub fn system(&self) -> ActorSystemGeneric<StdToolbox> {
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
  pub fn spawn_child<C>(
    &self,
    typed_props: &TypedProps<C>,
  ) -> Result<TypedChildRef<C>, SpawnError>
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
  pub fn spawn_child_watched<C>(
    &self,
    typed_props: &TypedProps<C>,
  ) -> Result<TypedChildRef<C>, SpawnError>
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
  pub const fn as_core(&self) -> &CoreTypedActorContextGeneric<'a, M, StdToolbox> {
    &self.inner
  }

  /// Provides mutable access to the underlying core typed context.
  pub fn as_core_mut(&mut self) -> &mut CoreTypedActorContextGeneric<'a, M, StdToolbox> {
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

impl<'a, M> DerefMut for TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
