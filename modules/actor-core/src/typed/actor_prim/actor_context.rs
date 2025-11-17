//! Typed actor context wrapper.

use core::{future::Future, marker::PhantomData, ptr::NonNull};

use fraktor_utils_core_rs::core::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  actor_prim::{ActorContextGeneric, Pid, PipeSpawnError},
  error::SendError,
  messaging::AnyMessageGeneric,
  spawn::SpawnError,
  typed::{
    TypedActorSystemGeneric,
    actor_prim::{actor_ref::TypedActorRefGeneric, child_ref::TypedChildRefGeneric},
    message_adapter::{AdaptMessage, AdapterError, AdapterFailure, MessageAdapterRegistry},
    props::TypedPropsGeneric,
  },
};

/// Provides typed helpers around the untyped [`ActorContextGeneric`].
pub struct TypedActorContextGeneric<'a, M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:    NonNull<ActorContextGeneric<'a, TB>>,
  adapters: Option<NonNull<MessageAdapterRegistry<M, TB>>>,
  _marker:  PhantomData<(&'a mut ActorContextGeneric<'a, TB>, M)>,
}

/// Type alias for [TypedActorContextGeneric] with the default [NoStdToolbox].
pub type TypedActorContext<'a, M> = TypedActorContextGeneric<'a, M, NoStdToolbox>;

impl<'a, M, TB> TypedActorContextGeneric<'a, M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a typed wrapper from the provided untyped context.
  pub(crate) fn from_untyped(
    inner: &mut ActorContextGeneric<'a, TB>,
    adapters: Option<&mut MessageAdapterRegistry<M, TB>>,
  ) -> Self {
    Self { inner: NonNull::from(inner), adapters: adapters.map(NonNull::from), _marker: PhantomData }
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
  pub fn system(&self) -> TypedActorSystemGeneric<M, TB> {
    TypedActorSystemGeneric::from_untyped(self.inner().system().clone())
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

  /// Spawns a typed child actor using the provided typed props
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_child<C>(
    &self,
    typed_props: &TypedPropsGeneric<C, TB>,
  ) -> Result<TypedChildRefGeneric<C, TB>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child(typed_props.to_untyped())?;
    Ok(TypedChildRefGeneric::from_untyped(child))
  }

  /// Spawns a typed child actor and automatically watches it.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned or watched.
  pub fn spawn_child_watched<C>(
    &self,
    typed_props: &TypedPropsGeneric<C, TB>,
  ) -> Result<TypedChildRefGeneric<C, TB>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child_watched(typed_props.to_untyped())?;
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

  fn registry_ptr(&self) -> Result<NonNull<MessageAdapterRegistry<M, TB>>, AdapterError> {
    self.adapters.ok_or(AdapterError::RegistryUnavailable)
  }

  /// Registers a message adapter for the specified payload type.
  ///
  /// # Errors
  ///
  /// Returns an error if the registry is unavailable or if registration fails.
  pub fn message_adapter<U, F>(&mut self, adapter: F) -> Result<TypedActorRefGeneric<U, TB>, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    let ctx_ptr = self.inner;
    let registry_ptr = self.registry_ptr()?;
    let actor_ref = unsafe {
      let ctx_ref = ctx_ptr.as_ref();
      let registry = &mut *registry_ptr.as_ptr();
      registry.register::<U, _>(ctx_ref, adapter)?
    };
    Ok(TypedActorRefGeneric::from_untyped(actor_ref))
  }

  /// Spawns a dedicated message adapter.
  ///
  /// # Errors
  ///
  /// Returns an error if the registry is unavailable or if adapter registration fails.
  pub fn spawn_message_adapter<U, F>(
    &mut self,
    _name: Option<&str>,
    adapter: F,
  ) -> Result<TypedActorRefGeneric<U, TB>, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    self.message_adapter(adapter)
  }

  /// Pipes a future back into the actor, adapting the response on the actor thread.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor is unavailable or stops before the task runs.
  pub fn pipe_to_self<U, E, Fut, MapOk, MapErr>(
    &mut self,
    future: Fut,
    map_ok: MapOk,
    map_err: MapErr,
  ) -> Result<(), PipeSpawnError>
  where
    Fut: Future<Output = Result<U, E>> + Send + 'static,
    U: Send + Sync + 'static,
    E: Send + Sync + 'static,
    MapOk: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static,
    MapErr: Fn(E) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    let mapped = async move {
      let outcome = future.await;
      let adapt = AdaptMessage::<M, TB>::new(outcome, move |result: Result<U, E>| match result {
        | Ok(value) => map_ok(value),
        | Err(error) => map_err(error),
      });
      AnyMessageGeneric::new(adapt)
    };
    self.inner().pipe_to_self(mapped, |message| message)
  }
}
