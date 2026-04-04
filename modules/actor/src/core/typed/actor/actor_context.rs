//! Typed actor context wrapper.

#[cfg(test)]
mod tests;

use alloc::{format, string::String, vec::Vec};
use core::{future::Future, marker::PhantomData, ptr::NonNull, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess, shared::Shared};

use crate::core::{
  kernel::{
    actor::{
      ActorContext, ChildRef, Pid,
      error::{ActorError, PipeSpawnError, SendError},
      messaging::{AnyMessage, AskError},
      spawn::SpawnError,
    },
    event::logging::LogLevel,
    pattern::install_ask_timeout,
    util::futures::ActorFutureListener,
  },
  typed::{
    TypedActorRef, TypedActorSystem,
    actor::{ask_on_context_error::AskOnContextError, child_ref::TypedChildRef},
    behavior::{Behavior, BehaviorDirective},
    dsl::{StatusReply, TypedAskError},
    internal::{ReceiveTimeoutConfig, TypedSchedulerGuard},
    message_adapter::{AdaptMessage, AdapterError, MessageAdapterBuilder, MessageAdapterRegistry},
    props::TypedProps,
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
  #[cfg(any(test, feature = "test-support"))]
  pub fn from_untyped(inner: &mut ActorContext<'a>, adapters: Option<&mut MessageAdapterRegistry<M>>) -> Self {
    Self::from_untyped_impl(inner, adapters)
  }

  /// Creates a typed wrapper from the provided untyped context.
  #[cfg(not(any(test, feature = "test-support")))]
  pub(crate) fn from_untyped(inner: &mut ActorContext<'a>, adapters: Option<&mut MessageAdapterRegistry<M>>) -> Self {
    Self::from_untyped_impl(inner, adapters)
  }

  fn from_untyped_impl(inner: &mut ActorContext<'a>, adapters: Option<&mut MessageAdapterRegistry<M>>) -> Self {
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

  /// Returns the metadata tags associated with the running actor.
  #[must_use]
  pub fn tags(&self) -> alloc::collections::BTreeSet<alloc::string::String> {
    self.inner().tags()
  }

  /// Sets a custom logger name for this actor context.
  ///
  /// Corresponds to Pekko's `ActorContext.setLoggerName(String)`.
  pub fn set_logger_name(&mut self, name: impl Into<alloc::string::String>) {
    self.inner_mut().set_logger_name(name);
  }

  /// Returns the custom logger name, if one has been configured.
  ///
  /// Corresponds to Pekko's `ActorContext.setLoggerName`.
  #[must_use]
  pub fn logger_name(&self) -> Option<&str> {
    self.inner().logger_name()
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
  pub fn spawn_child<C>(&mut self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner_mut().spawn_child(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Spawns an anonymous typed child actor from the given behavior.
  ///
  /// The child receives a system-generated name (no explicit name is set).
  /// Corresponds to Pekko's `ActorContext.spawnAnonymous`.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned.
  pub fn spawn_anonymous<C>(&mut self, behavior: &Behavior<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let initial_behavior = behavior.clone();
    let props = TypedProps::from_behavior_factory(move || initial_behavior.clone());
    self.spawn_child(&props)
  }

  /// Spawns a typed child actor and automatically watches it.
  ///
  /// # Errors
  ///
  /// Returns an error if the child actor cannot be spawned or watched.
  pub fn spawn_child_watched<C>(&mut self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner_mut().spawn_child_watched(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Watches the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch<C>(&mut self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().watch(target.as_untyped())
  }

  /// Watches the provided typed target with a custom message.
  ///
  /// When the target terminates, the provided `message` is delivered as a user message
  /// instead of a `Terminated` signal.
  ///
  /// # Errors
  ///
  /// Returns an error if the watch operation cannot be performed.
  pub fn watch_with<C>(&mut self, target: &TypedActorRef<C>, message: M) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().watch_with(target.as_untyped(), AnyMessage::new(message))
  }

  /// Stops watching the provided typed target.
  ///
  /// # Errors
  ///
  /// Returns an error if the unwatch operation cannot be performed.
  pub fn unwatch<C>(&mut self, target: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().unwatch(target.as_untyped())
  }

  /// Stops the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_self(&mut self) -> Result<(), SendError> {
    self.inner_mut().stop_self()
  }

  /// Stops the specified typed child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_child<C>(&mut self, child: &TypedChildRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().stop_child(child.as_untyped())
  }

  /// Stops the actor identified by the provided typed actor reference.
  ///
  /// Unlike [`stop_child`](Self::stop_child) which only accepts a child reference,
  /// this method can stop any actor in the system by its reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop_actor_by_ref<C>(&mut self, actor_ref: &TypedActorRef<C>) -> Result<(), SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().system().stop_actor(actor_ref.as_untyped().pid())
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
  pub fn stash(&mut self) -> Result<(), ActorError> {
    self.inner_mut().stash()
  }

  /// Stashes the currently processed message with an explicit capacity limit.
  ///
  /// # Errors
  ///
  /// Returns an error when no current message is active, when the stash reached `max_messages`,
  /// or when the actor cell is unavailable.
  pub fn stash_with_limit(&mut self, max_messages: usize) -> Result<(), ActorError> {
    self.inner_mut().stash_with_limit(max_messages)
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
  /// When the delegated behavior returns [`crate::core::typed::dsl::Behaviors::same`] or
  /// [`crate::core::typed::dsl::Behaviors::unhandled`], the delegated behavior itself becomes
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

  /// Forwards a typed message to the target, preserving the current sender.
  ///
  /// This is the user-facing fire-and-forget variant. Synchronous forwarding
  /// failures are observed internally and recorded via the system's send-error
  /// observation path.
  pub fn forward<C>(&mut self, target: &mut TypedActorRef<C>, message: C)
  where
    C: Send + Sync + 'static, {
    let _forward_result = self.try_forward(target, message);
  }

  /// Forwards a message to the target, preserving the current sender.
  ///
  /// This mirrors Pekko's `ActorRef.forward`. The message envelope retains the
  /// original sender so that the final recipient can reply to the original
  /// requester. Delivery is fire-and-forget.
  ///
  /// # Errors
  ///
  /// Returns an error if forwarding fails synchronously while enqueueing.
  pub fn try_forward<C>(
    &mut self,
    target: &mut TypedActorRef<C>,
    message: C,
  ) -> Result<(), crate::core::kernel::actor::error::SendError>
  where
    C: Send + Sync + 'static, {
    self.inner_mut().try_forward(target.as_untyped_mut(), AnyMessage::new(message))
  }

  /// Schedules a message to be sent to the specified target after `delay`.
  ///
  /// This mirrors Pekko's `ActorContext.scheduleOnce`. The message is
  /// delivered to `target` after the given delay using the system scheduler.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler cannot enqueue the command.
  pub fn schedule_once<C>(
    &self,
    delay: Duration,
    target: TypedActorRef<C>,
    message: C,
  ) -> Result<
    crate::core::kernel::actor::scheduler::SchedulerHandle,
    crate::core::kernel::actor::scheduler::SchedulerError,
  >
  where
    C: Send + Sync + 'static, {
    let scheduler = self.inner().system().scheduler();
    scheduler.with_write(|guard| TypedSchedulerGuard::new(guard).schedule_once(delay, target, message, None, None))
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
    self.inner_mut().pipe_to_self(mapped, |message| message)
  }

  /// Pipes the completion of an asynchronous computation to an external typed actor.
  ///
  /// Corresponds to Pekko's `PipeToSupport.pipeTo(recipient)`.
  /// Unlike [`pipe_to_self`](Self::pipe_to_self), this delivers the result directly
  /// to the recipient without going through the message adapter mechanism.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor is unavailable or stops before the task runs.
  pub fn pipe_to<R, U, E, Fut, MapOk, MapErr>(
    &mut self,
    future: Fut,
    recipient: &TypedActorRef<R>,
    map_ok: MapOk,
    map_err: MapErr,
  ) -> Result<(), PipeSpawnError>
  where
    R: Send + Sync + 'static,
    Fut: Future<Output = Result<U, E>> + Send + 'static,
    U: Send + Sync + 'static,
    E: Send + Sync + 'static,
    MapOk: FnOnce(U) -> Result<R, AdapterError> + Send + 'static,
    MapErr: FnOnce(E) -> Result<R, AdapterError> + Send + 'static, {
    let system = self.inner().system().clone();
    let pid = self.pid();
    let logger_name = self.logger_name().map(String::from);
    let mapped = async move {
      let value: Result<R, AdapterError> = match future.await {
        | Ok(v) => map_ok(v),
        | Err(e) => map_err(e),
      };
      match value {
        | Ok(msg) => Some(AnyMessage::new(msg)),
        | Err(adapter_err) => {
          system.emit_log(
            LogLevel::Warn,
            format!("typed pipe_to dropped message after adapter failure: {:?}", adapter_err),
            Some(pid),
            logger_name,
          );
          None
        },
      }
    };
    self.inner_mut().pipe_to(mapped, recipient.as_untyped(), |m| m)
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

  /// Sends a request to `target` and pipes the result back to this actor.
  ///
  /// Each ask spawns an independent future, so multiple concurrent asks
  /// with the same response type do not interfere. Timeout and failure
  /// are surfaced through `map_response` as `Err(TypedAskError)`.
  /// This mirrors Pekko's `pipeToSelf(target.ask(...))` pattern.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent or the pipe task cannot be spawned.
  pub fn ask<Req, Res, F, G>(
    &mut self,
    target: &mut TypedActorRef<Req>,
    create_request: F,
    map_response: G,
    timeout: Duration,
  ) -> Result<(), AskOnContextError>
  where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<Res>) -> Req,
    G: Fn(Result<Res, TypedAskError>) -> M + Send + Sync + 'static, {
    let ask_response = target.ask::<Res, _>(create_request);
    let (_, ask_future) = ask_response.into_parts();
    let raw_future = ask_future.into_inner();

    let system_state = self.inner().system().state();
    install_ask_timeout(&raw_future, &system_state, timeout);

    let listener = ActorFutureListener::new(raw_future);
    let map_fn = ArcShared::new(map_response);
    let map_fn_ok = map_fn.clone();
    let map_fn_err = map_fn;
    self.pipe_to_self(
      listener,
      move |message: AnyMessage| {
        let payload = message.payload_arc();
        drop(message);
        let typed_result: Result<Res, TypedAskError> = match payload.downcast::<Res>() {
          | Ok(concrete) => match concrete.try_unwrap() {
            | Ok(value) => Ok(value),
            | Err(_) => Err(TypedAskError::SharedReferences),
          },
          | Err(_) => Err(TypedAskError::TypeMismatch),
        };
        Ok(map_fn_ok(typed_result))
      },
      move |ask_error: AskError| Ok(map_fn_err(Err(TypedAskError::AskFailed(ask_error)))),
    )?;
    Ok(())
  }

  /// Sends a request expecting a [`StatusReply<Res>`] and pipes the result back.
  ///
  /// Success values are passed as `Ok(value)`, status errors as
  /// `Err(TypedAskError::StatusError(...))`. Timeout is delivered as
  /// `Err(TypedAskError::AskFailed(AskError::Timeout))`.
  /// This mirrors Pekko's `ActorContext.askWithStatus`.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent or the pipe task cannot be spawned.
  pub fn ask_with_status<Req, Res, F, G>(
    &mut self,
    target: &mut TypedActorRef<Req>,
    create_request: F,
    map_response: G,
    timeout: Duration,
  ) -> Result<(), AskOnContextError>
  where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<StatusReply<Res>>) -> Req,
    G: Fn(Result<Res, TypedAskError>) -> M + Send + Sync + 'static, {
    let ask_response = target.ask::<StatusReply<Res>, _>(create_request);
    let (_, ask_future) = ask_response.into_parts();
    let raw_future = ask_future.into_inner();

    let system_state = self.inner().system().state();
    install_ask_timeout(&raw_future, &system_state, timeout);

    let listener = ActorFutureListener::new(raw_future);
    let map_fn = ArcShared::new(map_response);
    let map_fn_ok = map_fn.clone();
    let map_fn_err = map_fn;
    self.pipe_to_self(
      listener,
      move |message: AnyMessage| {
        let payload = message.payload_arc();
        drop(message);
        let typed_result: Result<Res, TypedAskError> = match payload.downcast::<StatusReply<Res>>() {
          | Ok(concrete) => match concrete.try_unwrap() {
            | Ok(reply) => match StatusReply::<Res>::into_result(reply) {
              | Ok(value) => Ok(value),
              | Err(status_err) => Err(TypedAskError::StatusError(status_err)),
            },
            | Err(_) => Err(TypedAskError::SharedReferences),
          },
          | Err(_) => Err(TypedAskError::TypeMismatch),
        };
        Ok(map_fn_ok(typed_result))
      },
      move |ask_error: AskError| Ok(map_fn_err(Err(TypedAskError::AskFailed(ask_error)))),
    )?;
    Ok(())
  }
}
