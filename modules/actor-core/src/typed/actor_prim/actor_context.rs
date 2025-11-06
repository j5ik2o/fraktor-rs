//! Typed actor context wrapper.

use core::{marker::PhantomData, ptr::NonNull};

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  actor_prim::{ActorContextGeneric, Pid},
  error::SendError,
  messaging::AnyMessageGeneric,
  spawn::SpawnError,
  typed::{
    actor_prim::{actor_ref::TypedActorRefGeneric, child_ref::TypedChildRefGeneric},
    props::TypedPropsGeneric,
  },
};

/// Provides typed helpers around the untyped [`ActorContextGeneric`].
pub struct TypedActorContextGeneric<'a, M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:   NonNull<ActorContextGeneric<'a, TB>>,
  _marker: PhantomData<(&'a mut ActorContextGeneric<'a, TB>, M)>,
}

/// Type alias for [TypedActorContextGeneric] with the default [NoStdToolbox].
pub type TypedActorContext<'a, M> = TypedActorContextGeneric<'a, M, NoStdToolbox>;

impl<'a, M, TB> TypedActorContextGeneric<'a, M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a typed wrapper from the provided untyped context.
  pub(crate) fn from_untyped(inner: &mut ActorContextGeneric<'a, TB>) -> Self {
    Self { inner: NonNull::from(inner), _marker: PhantomData }
  }

  const fn inner(&self) -> &ActorContextGeneric<'a, TB> {
    // SAFETY: `inner` always points to a valid context for lifetime `'a`.
    unsafe { self.inner.as_ref() }
  }

  const fn inner_mut(&mut self) -> &mut ActorContextGeneric<'a, TB> {
    // SAFETY: The runtime guarantees exclusive access while executing actor code.
    unsafe { self.inner.as_mut() }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner().pid()
  }

  /// Returns the underlying actor system handle.
  #[must_use]
  pub fn system(&self) -> crate::system::ActorSystemGeneric<TB> {
    self.inner().system().clone()
  }

  /// Returns the typed self reference.
  #[must_use]
  pub fn self_ref(&self) -> TypedActorRefGeneric<M, TB> {
    TypedActorRefGeneric::from_untyped(self.inner().self_ref())
  }

  /// Sends a reply to the original sender.
  ///
  /// # Errors
  ///
  /// Returns an error if the sender is unavailable or the message cannot be delivered.
  pub fn reply<R>(&self, message: R) -> Result<(), SendError<TB>>
  where
    R: Send + Sync + 'static, {
    self.inner().reply(AnyMessageGeneric::new(message))
  }

  /// Spawns a typed child actor using the provided behavior.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_child<C>(&self, behavior: &TypedPropsGeneric<C, TB>) -> Result<TypedChildRefGeneric<C, TB>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child(behavior.props())?;
    Ok(TypedChildRefGeneric::from_untyped(child))
  }

  /// Spawns a typed child actor and automatically watches it.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned or watched.
  pub fn spawn_child_watched<C>(
      &self,
      behavior: &TypedPropsGeneric<C, TB>,
  ) -> Result<TypedChildRefGeneric<C, TB>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child_watched(behavior.props())?;
    Ok(TypedChildRefGeneric::from_untyped(child))
  }

  /// Watches the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch<C>(&self, target: &TypedActorRefGeneric<C, TB>) -> Result<(), SendError<TB>>
  where
    C: Send + Sync + 'static, {
    self.inner().watch(target.as_untyped())
  }

  /// Stops watching the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the unwatch operation cannot be performed.
  pub fn unwatch<C>(&self, target: &TypedActorRefGeneric<C, TB>) -> Result<(), SendError<TB>>
  where
    C: Send + Sync + 'static, {
    self.inner().unwatch(target.as_untyped())
  }

  /// Stops the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_self(&self) -> Result<(), SendError<TB>> {
    self.inner().stop_self()
  }

  /// Provides mutable access to the underlying untyped context.
  pub const fn as_untyped_mut(&mut self) -> &mut ActorContextGeneric<'a, TB> {
    self.inner_mut()
  }
}
