extern crate std;
use std::{
  marker::PhantomData,
  ops::{Deref, DerefMut},
  ptr::NonNull,
  vec::Vec,
};

use crate::{
  core::{
    actor::{ChildRef, Pid},
    error::SendError,
    spawn::SpawnError,
    typed::{TypedActorSystem, actor::TypedActorContext as CoreTypedActorContext},
  },
  std::typed::{
    TypedProps,
    actor::{TypedActorRef, TypedChildRef},
  },
};

/// Typed actor context wrapper for the standard runtime.
pub struct TypedActorContext<'ctx, 'inner, M>
where
  M: Send + Sync + 'static, {
  inner:   NonNull<CoreTypedActorContext<'inner, M>>,
  mutable: bool,
  _marker: PhantomData<(&'ctx CoreTypedActorContext<'inner, M>, M)>,
}

impl<'ctx, 'inner, M> TypedActorContext<'ctx, 'inner, M>
where
  M: Send + Sync + 'static,
{
  const fn inner(&self) -> &CoreTypedActorContext<'inner, M> {
    // SAFETY: `inner` always points to a valid context for lifetime `'ctx`.
    unsafe { self.inner.as_ref() }
  }

  fn inner_mut(&mut self) -> &mut CoreTypedActorContext<'inner, M> {
    assert!(self.mutable, "supervisor_strategy では読み取り専用 TypedActorContext を使用してください");
    // SAFETY: mutable instances are only constructed from exclusive references.
    unsafe { self.inner.as_mut() }
  }

  /// Builds a std-facing typed context wrapper from the core context.
  #[must_use]
  pub fn from_core_mut(core: &'ctx mut CoreTypedActorContext<'inner, M>) -> Self {
    Self { inner: NonNull::from(core), mutable: true, _marker: PhantomData }
  }

  /// Builds a read-only std-facing typed context wrapper from the core context.
  #[must_use]
  pub fn from_core(core: &'ctx CoreTypedActorContext<'inner, M>) -> Self {
    Self { inner: NonNull::from(core), mutable: false, _marker: PhantomData }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner().pid()
  }

  /// Returns the underlying actor system handle.
  #[must_use]
  pub fn system(&self) -> TypedActorSystem<M> {
    self.inner().system()
  }

  /// Returns the typed self reference.
  #[must_use]
  pub fn self_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_core(self.inner().self_ref())
  }

  /// Spawns a typed child actor using the provided typed props
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_child<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child(typed_props.as_core())?;
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
    let child = self.inner().spawn_child_watched(typed_props.as_core())?;
    Ok(TypedChildRef::from_core(child))
  }

  /// Watches the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().watch(target.as_core())
  }

  /// Stops watching the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the unwatch operation cannot be performed.
  pub fn unwatch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().unwatch(target.as_core())
  }

  /// Stops the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_self(&self) -> Result<(), SendError> {
    self.inner().stop_self()
  }

  /// Stops the specified typed child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_child<C>(&self, child: &TypedChildRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().stop_child(child.as_core())
  }

  /// Stops the actor identified by the provided typed actor reference.
  ///
  /// Unlike [`stop_child`](Self::stop_child) which only accepts a child reference,
  /// this method can stop any actor in the system by its reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_actor_by_ref<C>(&self, actor_ref: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().stop_actor_by_ref(actor_ref.as_core())
  }

  /// Returns the list of supervised children as untyped [`ChildRef`] values.
  ///
  /// Children may have different message types, so returning typed references
  /// is not feasible here. Use [`spawn_child`](Self::spawn_child) to obtain a
  /// typed [`TypedChildRef`](crate::core::typed::actor::child_ref::TypedChildRef).
  #[must_use]
  pub fn children(&self) -> Vec<ChildRef> {
    self.inner().children()
  }

  /// Returns the child with the specified name as an untyped [`ChildRef`], if present.
  ///
  /// See [`children`](Self::children) for why this returns an untyped reference.
  #[must_use]
  pub fn child(&self, name: &str) -> Option<ChildRef> {
    self.inner().child(name)
  }

  /// Creates a fluent builder for registering a typed message adapter.
  #[must_use]
  pub fn message_adapter_builder<U>(
    &mut self,
  ) -> crate::core::typed::message_adapter::MessageAdapterBuilder<'_, 'inner, M, U>
  where
    U: Send + Sync + 'static, {
    self.inner_mut().message_adapter_builder()
  }

  /// Provides access to the underlying core typed context.
  #[must_use]
  pub const fn as_core(&self) -> &CoreTypedActorContext<'inner, M> {
    self.inner()
  }

  /// Provides mutable access to the underlying core typed context.
  pub fn as_core_mut(&mut self) -> &mut CoreTypedActorContext<'inner, M> {
    self.inner_mut()
  }
}

impl<'inner, M> Deref for TypedActorContext<'_, 'inner, M>
where
  M: Send + Sync + 'static,
{
  type Target = CoreTypedActorContext<'inner, M>;

  fn deref(&self) -> &Self::Target {
    self.inner()
  }
}

impl<M> DerefMut for TypedActorContext<'_, '_, M>
where
  M: Send + Sync + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.inner_mut()
  }
}
