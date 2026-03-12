//! Typed actor context wrapper.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::{future::Future, marker::PhantomData, ptr::NonNull, time::Duration};

use crate::core::{
  actor::{ActorContext, ChildRef, Pid, PipeSpawnError},
  error::{ActorError, SendError},
  messaging::AnyMessage,
  spawn::SpawnError,
  typed::{
    TypedActorSystem,
    actor::{actor_ref::TypedActorRef, child_ref::TypedChildRef},
    behavior::{Behavior, BehaviorDirective},
    message_adapter::{AdaptMessage, AdapterError, MessageAdapterBuilder, MessageAdapterRegistry},
    props::TypedProps,
    receive_timeout_config::ReceiveTimeoutConfig,
  },
};

/// Provides typed helpers around the untyped [`ActorContext`].
pub struct TypedActorContext<'a, M>
where
  M: Send + Sync + 'static, {
  inner:           NonNull<ActorContext<'a>>,
  adapters:        Option<NonNull<MessageAdapterRegistry<M>>>,
  receive_timeout: Option<NonNull<Option<ReceiveTimeoutConfig<M>>>>,
  _marker:         PhantomData<(&'a mut ActorContext<'a>, M)>,
}

impl<'a, M> TypedActorContext<'a, M>
where
  M: Send + Sync + 'static,
{
  /// Creates a typed wrapper from the provided untyped context.
  pub(crate) fn from_untyped(inner: &mut ActorContext<'a>, adapters: Option<&mut MessageAdapterRegistry<M>>) -> Self {
    Self {
      inner:           NonNull::from(inner),
      adapters:        adapters.map(NonNull::from),
      receive_timeout: None,
      _marker:         PhantomData,
    }
  }

  /// Attaches a receive timeout state reference to this context.
  pub(crate) fn with_receive_timeout(mut self, state: &mut Option<ReceiveTimeoutConfig<M>>) -> Self {
    self.receive_timeout = Some(NonNull::from(state));
    self
  }

  const fn inner(&self) -> &ActorContext<'a> {
    // SAFETY: `inner` always points to a valid context for lifetime `'a`.
    unsafe { self.inner.as_ref() }
  }

  const fn inner_mut(&mut self) -> &mut ActorContext<'a> {
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
  pub fn system(&self) -> TypedActorSystem<M> {
    TypedActorSystem::from_untyped(self.inner().system().clone())
  }

  /// Returns the typed self reference.
  #[must_use]
  pub fn self_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_untyped(self.inner().self_ref())
  }

  /// Spawns a typed child actor using the provided typed props
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_child<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Spawns a typed child actor and automatically watches it.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned or watched.
  pub fn spawn_child_watched<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner().spawn_child_watched(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Watches the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().watch(target.as_untyped())
  }

  /// Watches the provided typed target with a custom message.
  ///
  /// When the target terminates, the provided `message` is delivered as a user message
  /// instead of a `Terminated` signal.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch_with<C>(&self, target: &TypedActorRef<C>, message: M) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().watch_with(target.as_untyped(), AnyMessage::new(message))
  }

  /// Stops watching the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the unwatch operation cannot be performed.
  pub fn unwatch<C>(&self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner().unwatch(target.as_untyped())
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
    self.inner().stop_child(child.as_untyped())
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
    self.inner().system().stop_actor(actor_ref.as_untyped().pid())
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

  /// Stashes the currently processed message for deferred handling.
  ///
  /// # Errors
  ///
  /// Returns an error when no current message is active or actor cell access fails.
  pub fn stash(&self) -> Result<(), ActorError> {
    self.inner().stash()
  }

  /// Stashes the currently processed message with an explicit capacity limit.
  ///
  /// # Errors
  ///
  /// Returns an error when no current message is active, when the stash reached `max_messages`,
  /// or when the actor cell is unavailable.
  pub fn stash_with_limit(&self, max_messages: usize) -> Result<(), ActorError> {
    self.inner().stash_with_limit(max_messages)
  }

  /// Re-enqueues the oldest stashed message back to the actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when actor cell access or unstash dispatch fails.
  pub fn unstash(&self) -> Result<usize, ActorError> {
    self.inner().unstash()
  }

  /// Re-enqueues all stashed messages back to the actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when actor cell access or unstash dispatch fails.
  pub fn unstash_all(&self) -> Result<usize, ActorError> {
    self.inner().unstash_all()
  }

  /// Delegates the provided message to another behavior and returns the resulting next behavior.
  ///
  /// When the delegated behavior returns [`crate::core::typed::Behaviors::same`] or
  /// [`crate::core::typed::Behaviors::unhandled`], the delegated behavior itself becomes
  /// the next active behavior, matching Pekko's `ActorContext.delegate` contract.
  ///
  /// # Errors
  ///
  /// Returns an error if the delegated behavior fails while handling the message.
  pub fn delegate(&mut self, mut delegatee: Behavior<M>, msg: &M) -> Result<Behavior<M>, ActorError> {
    let next = delegatee.handle_message(self, msg)?;
    match next.directive() {
      | BehaviorDirective::Same | BehaviorDirective::Unhandled => Ok(delegatee),
      | _ => Ok(next),
    }
  }

  /// Provides mutable access to the underlying untyped context.
  pub const fn as_untyped_mut(&mut self) -> &mut ActorContext<'a> {
    self.inner_mut()
  }

  fn registry_ptr(&self) -> Result<NonNull<MessageAdapterRegistry<M>>, AdapterError> {
    self.adapters.ok_or(AdapterError::RegistryUnavailable)
  }

  /// Creates a fluent builder for registering a message adapter.
  #[must_use]
  pub const fn message_adapter_builder<U>(&mut self) -> MessageAdapterBuilder<'_, 'a, M, U>
  where
    U: Send + Sync + 'static, {
    MessageAdapterBuilder::new(self)
  }

  /// Registers a message adapter for the specified payload type.
  ///
  /// # Errors
  ///
  /// Returns an error if the registry is unavailable or if registration fails.
  pub fn message_adapter<U, F>(&mut self, adapter: F) -> Result<TypedActorRef<U>, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
    let ctx_ptr = self.inner;
    let registry_ptr = self.registry_ptr()?;
    let actor_ref = unsafe {
      let ctx_ref = ctx_ptr.as_ref();
      let registry = &mut *registry_ptr.as_ptr();
      registry.register::<U, _>(ctx_ref, adapter)?
    };
    Ok(TypedActorRef::from_untyped(actor_ref))
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
  ) -> Result<TypedActorRef<U>, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
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
    MapOk: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static,
    MapErr: Fn(E) -> Result<M, AdapterError> + Send + Sync + 'static, {
    let mapped = async move {
      let outcome = future.await;
      let adapt = AdaptMessage::<M>::new(outcome, move |result: Result<U, E>| match result {
        | Ok(value) => map_ok(value),
        | Err(error) => map_err(error),
      });
      AnyMessage::new(adapt)
    };
    self.inner().pipe_to_self(mapped, |message| message)
  }

  /// Configures an idle timeout that sends `message` when no messages are received within
  /// `timeout`.
  ///
  /// The timer resets on every message delivery. Calling this again replaces the previous
  /// configuration. Use [`cancel_receive_timeout`](Self::cancel_receive_timeout) to disable.
  pub fn set_receive_timeout<F>(&mut self, timeout: Duration, message_factory: F)
  where
    F: Fn() -> M + Send + Sync + 'static, {
    if let Some(mut ptr) = self.receive_timeout {
      // SAFETY: The pointer is valid for the duration of the actor's message processing.
      let state = unsafe { ptr.as_mut() };
      *state = Some(ReceiveTimeoutConfig::new(timeout, message_factory));
    }
  }

  /// Disables the receive timeout previously set via
  /// [`set_receive_timeout`](Self::set_receive_timeout).
  pub fn cancel_receive_timeout(&mut self) {
    if let Some(mut ptr) = self.receive_timeout {
      // SAFETY: The pointer is valid for the duration of the actor's message processing.
      let state = unsafe { ptr.as_mut() };
      *state = None;
    }
  }
}
