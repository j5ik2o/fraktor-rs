//! Runtime container responsible for executing an actor instance.

#[cfg(test)]
mod tests;

use alloc::{
  borrow::ToOwned,
  boxed::Box,
  collections::{BTreeSet, VecDeque},
  format,
  string::String,
  vec,
  vec::Vec,
};
use core::{mem, task::Poll, time::Duration};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex, WeakShared};
use portable_atomic::{AtomicBool, Ordering};

use crate::core::{
  kernel::{
    actor::{
      Actor, ActorContext, ActorShared, Pid, STASH_OVERFLOW_REASON, STASH_REQUIRES_DEQUE_REASON,
      actor_context::ReceiveTimeoutState,
      actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared},
      context_pipe::{ContextPipeFuture, ContextPipeTask, ContextPipeTaskId},
      error::{ActorError, PipeSpawnError},
      lifecycle::{LifecycleEvent, LifecycleStage},
      messaging::{
        ActorIdentity, AnyMessage, Identify,
        message_invoker::{MessageInvoker, MessageInvokerPipeline, MessageInvokerShared},
        system_message::{FailureMessageSnapshot, FailurePayload, SystemMessage},
      },
      props::{ActorFactoryShared, Props},
      scheduler::{SchedulerCommand, SchedulerError, SchedulerHandle, SchedulerShared},
      spawn::SpawnError,
      supervision::{RestartStatistics, SupervisorDirective, SupervisorStrategyKind},
    },
    dispatch::{
      dispatcher::{DEFAULT_DISPATCHER_ID, Dispatchers, MessageDispatcherShared},
      mailbox::{Mailbox, MailboxCapacity, MailboxInstrumentation, metrics_event::MailboxPressureEvent},
    },
    event::{logging::LogLevel, stream::EventStreamEvent},
    system::{
      ActorSystem,
      guardian::GuardianKind,
      lock_provider::MailboxSharedSet,
      state::{SystemStateShared, SystemStateWeak, system_state::FailureOutcome},
    },
  },
  typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
};

struct ActorCellState {
  children:               Vec<Pid>,
  child_stats:            Vec<(Pid, RestartStatistics)>,
  watchers:               Vec<Pid>,
  watch_with_messages:    Vec<(Pid, AnyMessage)>,
  stashed_messages:       VecDeque<AnyMessage>,
  timer_handles:          Vec<(String, SchedulerHandle)>,
  pipe_tasks:             Vec<ContextPipeTask>,
  adapter_handles:        Vec<AdapterRefHandle>,
  adapter_handle_counter: u64,
  pipe_task_counter:      u64,
}

impl ActorCellState {
  const fn new() -> Self {
    Self {
      children:               Vec::new(),
      child_stats:            Vec::new(),
      watchers:               Vec::new(),
      watch_with_messages:    Vec::new(),
      stashed_messages:       VecDeque::new(),
      timer_handles:          Vec::new(),
      pipe_tasks:             Vec::new(),
      adapter_handles:        Vec::new(),
      adapter_handle_counter: 0,
      pipe_task_counter:      0,
    }
  }
}

/// Runtime container responsible for executing an actor instance.
///
/// ```compile_fail
/// use fraktor_actor_core_rs::core::kernel::actor::ActorCell;
///
/// fn read_dispatcher_id(cell: &ActorCell) {
///   let _ = cell.dispatcher_id();
/// }
/// ```
pub struct ActorCell {
  pid:             Pid,
  parent:          Option<Pid>,
  name:            String,
  tags:            BTreeSet<String>,
  system:          SystemStateWeak,
  factory:         ActorFactoryShared,
  actor:           ActorShared,
  pipeline:        MessageInvokerPipeline,
  /// Interior-mutable mailbox slot.
  ///
  /// `ActorCell::create` builds the mailbox eagerly and immediately calls
  /// [`Self::install_mailbox`], so by the time the cell is observable from
  /// outside the slot is always populated. The interior mutability is
  /// confined to this single slot to leave the rest of `ActorCell` as plain
  /// owned state — it is the only seam needed for the Pekko-style 2-phase
  /// `attach` flow where the dispatcher creates the mailbox after the cell
  /// exists. The slot uses `SpinSyncMutex` so it stays `no_std` compatible.
  mailbox:         SpinSyncMutex<Option<ArcShared<Mailbox>>>,
  dispatcher_id:   String,
  /// Handle to the new-dispatcher tree that owns the cell.
  ///
  /// Every cell is attached to a [`MessageDispatcherShared`] when it is
  /// constructed; `SystemStateShared::remove_cell` calls `detach` on the
  /// inhabitants counter when the cell is dropped.
  new_dispatcher:  MessageDispatcherShared,
  sender:          ActorRefSenderShared,
  receive_timeout: SharedLock<Option<ReceiveTimeoutState>>,
  state:           SharedLock<ActorCellState>,
  terminated:      AtomicBool,
}

unsafe impl Send for ActorCell {}
unsafe impl Sync for ActorCell {}

impl ActorCell {
  /// Upgrades the weak system reference to a strong reference.
  ///
  /// # Panics
  ///
  /// Panics if the system state has already been dropped.
  #[allow(clippy::expect_used)]
  pub(crate) fn system(&self) -> SystemStateShared {
    self.system.upgrade().expect("system state has been dropped")
  }

  /// Returns the scheduler handle owned by the underlying actor system.
  ///
  /// # Panics
  ///
  /// Panics if the system state has already been dropped.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.system().scheduler()
  }

  fn make_context(&self) -> ActorContext<'_> {
    let system = ActorSystem::from_state(self.system());
    ActorContext::new(&system, self.pid).with_receive_timeout_state(&self.receive_timeout)
  }

  /// Creates a new actor cell using the provided runtime state and props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the props or system state cannot satisfy the
  /// requested spawn (for example, missing dispatcher configurator or
  /// invalid mailbox id).
  #[allow(clippy::needless_pass_by_value)]
  pub fn create(
    system: SystemStateShared,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &Props,
  ) -> Result<ArcShared<Self>, SpawnError> {
    let mailbox_id = props.mailbox_id();

    let mailbox_config = if let Some(id) = mailbox_id {
      system.resolve_mailbox(id).map_err(|error| SpawnError::invalid_props(alloc::format!("{error:?}")))?
    } else {
      props.mailbox_config().clone()
    };

    let dispatcher_id = Self::resolve_dispatcher_id(&system, parent, props)?;
    // The dispatcher tree owns the ActorRef sender path. A configurator must
    // be registered for the resolved id (`ActorSystemConfig::default()` seeds
    // the in-process inline configurator).
    let new_dispatcher = system.resolve_dispatcher(&dispatcher_id).ok_or_else(|| {
      SpawnError::invalid_props(alloc::format!("no dispatcher configurator registered for id `{dispatcher_id}`"))
    })?;
    // Optional runtime override for `*Shared` construction. `None` is the
    // common path: shared wrappers are built via the workspace's compile-time
    // selected default lock driver. `Some` is reserved for tests / debug
    // builds that install `DebugActorLockProvider` (or a `parking_lot`
    // variant) at the actor system boundary.
    let lock_provider = system.lock_provider();

    // Give the dispatcher a chance to supply its own mailbox (e.g.,
    // `BalancingDispatcher` hands out sharing mailboxes that all wrap its
    // single team queue). Dispatchers that want per-actor queues return
    // `None` and `ActorCell` falls back to the `MailboxConfig`-driven path.
    let mailbox = if let Some(shared_mailbox) = new_dispatcher.try_create_shared_mailbox() {
      shared_mailbox
    } else if let Some(id) = mailbox_id {
      let queue =
        system.create_mailbox_queue(id).map_err(|error| SpawnError::invalid_props(alloc::format!("{error:?}")))?;
      let shared_set = match lock_provider.as_ref() {
        | Some(provider) => provider.create_mailbox_shared_set(),
        | None => MailboxSharedSet::with_builtin_lock(),
      };
      ArcShared::new(Mailbox::new_with_queue_and_shared_set(mailbox_config.policy(), queue, &shared_set))
    } else {
      let shared_set = match lock_provider.as_ref() {
        | Some(provider) => provider.create_mailbox_shared_set(),
        | None => MailboxSharedSet::with_builtin_lock(),
      };
      ArcShared::new(
        Mailbox::new_from_config_with_shared_set(&mailbox_config, &shared_set)
          .map_err(|error| SpawnError::invalid_props(alloc::format!("{error}")))?,
      )
    };
    {
      let policy = mailbox_config.policy();
      let capacity = match policy.capacity() {
        | MailboxCapacity::Bounded { capacity } => Some(capacity.get()),
        | MailboxCapacity::Unbounded => None,
      };
      let throughput = policy.throughput_limit().map(|limit| limit.get());
      let warn_threshold = mailbox_config.warn_threshold().map(|threshold| threshold.get());
      let instrumentation = MailboxInstrumentation::new(system.clone(), pid, capacity, throughput, warn_threshold);
      mailbox.set_instrumentation(instrumentation);
    }
    use crate::core::kernel::dispatch::dispatcher::DispatcherSender;
    let sender = {
      let dispatcher_sender: Box<dyn ActorRefSender> =
        Box::new(DispatcherSender::new(new_dispatcher.clone(), mailbox.clone()));
      match lock_provider.as_ref() {
        | Some(provider) => provider.create_actor_ref_sender_shared(dispatcher_sender),
        | None => ActorRefSenderShared::from_shared_lock(SharedLock::new(dispatcher_sender)),
      }
    };
    let Some(factory) = props.factory().cloned() else {
      return Err(SpawnError::invalid_props("actor factory is required"));
    };
    let actor = ActorShared::new(factory.with_write(|f| f.create()));
    let receive_timeout = SharedLock::new_with_driver::<SpinSyncMutex<_>>(None);
    let state = SharedLock::new_with_driver::<SpinSyncMutex<_>>(ActorCellState::new());

    let tags = props.tags().clone();
    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      tags,
      system: system.downgrade(),
      factory,
      actor,
      pipeline: MessageInvokerPipeline::new(),
      mailbox: SpinSyncMutex::new(None),
      dispatcher_id,
      new_dispatcher,
      sender,
      receive_timeout,
      state,
      terminated: AtomicBool::new(false),
    });

    // Install the eagerly-built mailbox into the cell's interior-mutable slot.
    // The contract is "install once": every later call to `cell.mailbox()`
    // returns this `ArcShared<Mailbox>`. This is the single seam used to
    // satisfy the Pekko-style 2-phase init where the dispatcher is responsible
    // for the mailbox; until that flow lands the cell still pre-builds the
    // mailbox and installs it itself, but the slot makes the `install_mailbox`
    // contract observable to tests and to future dispatcher-side
    // `create_mailbox` callers.
    cell.install_mailbox(mailbox);

    {
      // Install the message invoker on the mailbox so the new dispatcher's
      // `Mailbox::run` drain loop can deliver user/system messages back to
      // this actor cell. The invoker holds a weak reference to the cell to
      // break the ActorCell → Mailbox → Invoker → ActorCell ownership cycle.
      let mailbox_handle = cell.mailbox();
      let invoker: MessageInvokerShared =
        MessageInvokerShared::new(Box::new(ActorCellInvoker { cell: cell.downgrade() }));
      mailbox_handle.install_invoker(invoker);
      // Late-bind the weak actor handle to the mailbox so `Mailbox::run` can
      // early-return after the cell drops, and so detach paths can call
      // `Mailbox::clean_up` without re-deriving the back-reference.
      let _previous_actor = mailbox_handle.install_actor(cell.downgrade());
    }

    // Register the new dispatcher attach so the inhabitants counter matches the
    // cell lifetime; `SystemStateShared::remove_cell` calls `detach` on drop.
    cell.new_dispatcher.attach(&cell)?;

    Ok(cell)
  }

  /// Recreates the actor instance from the stored factory.
  fn recreate_actor(&self) {
    self.actor.with_write(|actor| {
      *actor = self.factory.with_write(|f| f.create());
    });
  }

  fn resolve_dispatcher_id(
    system: &SystemStateShared,
    parent: Option<Pid>,
    props: &Props,
  ) -> Result<String, SpawnError> {
    if props.dispatcher_same_as_parent() {
      if let Some(parent_pid) = parent {
        let parent_cell = system.cell(&parent_pid).ok_or_else(|| SpawnError::invalid_props("parent cell missing"))?;
        return Ok(parent_cell.dispatcher_id().to_owned());
      }
      return Ok(DEFAULT_DISPATCHER_ID.to_owned());
    }

    let dispatcher_id = props.dispatcher_id().unwrap_or(DEFAULT_DISPATCHER_ID);
    let normalized = Dispatchers::normalize_dispatcher_id(dispatcher_id);
    if system.resolve_dispatcher(normalized).is_none() {
      return Err(SpawnError::invalid_props(alloc::format!("dispatcher `{normalized}` is not registered")));
    }
    Ok(normalized.to_owned())
  }

  /// Returns the pid associated with the cell.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical actor name.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Returns the parent pid if registered.
  #[must_use]
  pub const fn parent(&self) -> Option<Pid> {
    self.parent
  }

  /// Returns the metadata tags associated with this actor.
  #[must_use]
  pub const fn tags(&self) -> &BTreeSet<String> {
    &self.tags
  }

  /// Installs the mailbox into the cell's interior-mutable slot.
  ///
  /// Calling this twice on the same cell is a programmer error: the contract
  /// is "install once" so callers can rely on `cell.mailbox()` returning a
  /// stable handle for the rest of the cell's lifetime. The double-install
  /// case panics in debug builds and overwrites in release.
  ///
  /// # Panics
  ///
  /// Panics in debug builds when called more than once for the same cell.
  pub fn install_mailbox(&self, mailbox: ArcShared<Mailbox>) {
    let mut slot = self.mailbox.lock();
    debug_assert!(slot.is_none(), "ActorCell::install_mailbox called twice for the same cell");
    *slot = Some(mailbox);
  }

  /// Returns a handle to the mailbox managed by this cell.
  ///
  /// # Panics
  ///
  /// Panics when the cell has not yet been initialised via
  /// [`Self::install_mailbox`]. In normal flow `ActorCell::create` performs
  /// the install before returning the cell, so external callers always
  /// observe a populated slot.
  #[must_use]
  #[allow(clippy::expect_used)]
  pub fn mailbox(&self) -> ArcShared<Mailbox> {
    self.mailbox.lock().clone().expect("mailbox not installed yet")
  }

  /// Returns the new-dispatcher handle owned by this cell.
  #[must_use]
  pub fn new_dispatcher_shared(&self) -> MessageDispatcherShared {
    self.new_dispatcher.clone()
  }

  /// Returns the resolved dispatcher identifier associated with this cell.
  #[must_use]
  pub(crate) fn dispatcher_id(&self) -> &str {
    &self.dispatcher_id
  }

  /// Returns a sender handle targeting this actor cell's mailbox.
  #[must_use]
  pub(crate) fn mailbox_sender(&self) -> ActorRefSenderShared {
    self.sender.clone()
  }

  /// Produces an actor reference targeting this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRef {
    ActorRef::from_shared(self.pid, self.sender.clone(), &self.system())
  }

  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    self.state.with_write(|state| {
      if !state.children.contains(&pid) {
        state.children.push(pid);
      }
      find_or_insert_stats(&mut state.child_stats, pid);
    });
  }

  /// Removes a child pid from supervision tracking.
  pub fn unregister_child(&self, pid: &Pid) {
    self.state.with_write(|state| {
      state.children.retain(|child| child != pid);
      state.child_stats.retain(|(child, _)| child != pid);
    });
  }

  fn stop_child(&self, pid: Pid) {
    let should_stop = self.state.with_read(|state| state.children.contains(&pid));
    if should_stop && let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Stop) {
      self.system().record_send_error(Some(pid), &send_error);
    }
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.children.clone())
  }

  pub(crate) fn snapshot_child_restart_stats(&self, pid: Pid) -> Option<RestartStatistics> {
    self
      .state
      .with_read(|state| state.child_stats.iter().find(|(child, _)| *child == pid).map(|(_, record)| record.clone()))
  }

  fn mark_terminated(&self) {
    self.terminated.store(true, Ordering::Release);
    self.drop_adapter_refs();
    self.drop_pipe_tasks();
  }

  fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  pub(crate) fn handle_watch(&self, watcher: Pid) {
    if self.is_terminated() {
      if let Err(send_error) = self.system().send_system_message(watcher, SystemMessage::Terminated(self.pid)) {
        self.system().record_send_error(Some(watcher), &send_error);
      }
      return;
    }

    let notify_immediately = self.state.with_write(|state| {
      if self.is_terminated() {
        return true;
      }
      if !state.watchers.contains(&watcher) {
        state.watchers.push(watcher);
      }
      false
    });
    if notify_immediately
      && let Err(send_error) = self.system().send_system_message(watcher, SystemMessage::Terminated(self.pid))
    {
      self.system().record_send_error(Some(watcher), &send_error);
    }
  }

  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.state.with_write(|state| state.watchers.retain(|pid| *pid != watcher));
  }

  /// Stashes a user message with an explicit stash capacity limit.
  ///
  /// # Errors
  ///
  /// Returns an overflow error when the stash already reached `max_messages`.
  pub(crate) fn stash_message_with_limit(&self, message: AnyMessage, max_messages: usize) -> Result<(), ActorError> {
    if self.mailbox().user_deque().is_none() {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    }
    self.state.with_write(|state| {
      if state.stashed_messages.len() >= max_messages {
        return Err(ActorError::recoverable(STASH_OVERFLOW_REASON));
      }
      state.stashed_messages.push_back(message);
      Ok(())
    })
  }

  /// Returns the number of messages currently held in the stash.
  #[must_use]
  pub(crate) fn stashed_message_len(&self) -> usize {
    self.state.with_read(|state| state.stashed_messages.len())
  }

  /// Applies a read-only closure to the current stashed messages.
  pub(crate) fn with_stashed_messages<R>(&self, f: impl FnOnce(&VecDeque<AnyMessage>) -> R) -> R {
    self.state.with_read(|state| f(&state.stashed_messages))
  }

  /// Removes all currently stashed messages and returns how many were dropped.
  #[must_use]
  pub(crate) fn clear_stashed_messages(&self) -> usize {
    self.state.with_write(|state| {
      let count = state.stashed_messages.len();
      state.stashed_messages.clear();
      count
    })
  }

  fn take_timer_handle(&self, key: &str) -> Option<SchedulerHandle> {
    self.state.with_write(|state| {
      let index = state.timer_handles.iter().position(|(existing, _)| existing == key)?;
      let (_, handle) = state.timer_handles.swap_remove(index);
      Some(handle)
    })
  }

  fn store_timer_handle(&self, key: String, handle: SchedulerHandle) {
    self.state.with_write(|state| {
      if let Some((_, existing_handle)) = state.timer_handles.iter_mut().find(|(existing, _)| existing == &key) {
        *existing_handle = handle;
      } else {
        state.timer_handles.push((key, handle));
      }
    });
  }

  fn schedule_timer_command(
    &self,
    key: String,
    initial_delay: Duration,
    command: SchedulerCommand,
    interval: Option<Duration>,
    fixed_rate: bool,
  ) -> Result<(), SchedulerError> {
    self.cancel_timer(&key);
    let scheduler = self.system().scheduler();
    let handle = scheduler.with_write(|scheduler| match (interval, fixed_rate) {
      | (Some(duration), true) => scheduler.schedule_at_fixed_rate(initial_delay, duration, command),
      | (Some(duration), false) => scheduler.schedule_with_fixed_delay(initial_delay, duration, command),
      | (None, _) => scheduler.schedule_once(initial_delay, command),
    })?;
    self.store_timer_handle(key, handle);
    Ok(())
  }

  /// Schedules a one-shot timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_single_timer(
    &self,
    key: String,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, delay, command, None, false)
  }

  /// Schedules a fixed-delay timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_fixed_delay_timer(
    &self,
    key: String,
    message: AnyMessage,
    initial_delay: Duration,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, initial_delay, command, Some(delay), false)
  }

  /// Schedules a fixed-rate timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_fixed_rate_timer(
    &self,
    key: String,
    message: AnyMessage,
    initial_delay: Duration,
    interval: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, initial_delay, command, Some(interval), true)
  }

  /// Returns whether the timer associated with `key` is currently active.
  #[must_use]
  pub(crate) fn is_timer_active(&self, key: &str) -> bool {
    self.state.with_read(|state| {
      state
        .timer_handles
        .iter()
        .find(|(existing, _)| existing == key)
        .is_some_and(|(_, handle)| !handle.is_cancelled() && !handle.is_completed())
    })
  }

  /// Cancels the timer associated with `key`.
  pub(crate) fn cancel_timer(&self, key: &str) {
    let Some(handle) = self.take_timer_handle(key) else {
      return;
    };
    self.system().scheduler().with_write(|scheduler| {
      scheduler.cancel(&handle);
    });
  }

  /// Cancels every tracked timer for this actor.
  pub(crate) fn cancel_all_timers(&self) {
    let handles = self.state.with_write(|state| mem::take(&mut state.timer_handles));
    if handles.is_empty() {
      return;
    }
    self.system().scheduler().with_write(|scheduler| {
      for (_, handle) in &handles {
        scheduler.cancel(handle);
      }
    });
  }

  /// Re-enqueues the oldest stashed user message back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when mailbox enqueue fails. Remaining messages stay stashed.
  pub(crate) fn unstash_message(&self) -> Result<usize, ActorError> {
    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let message = self.state.with_write(|state| state.stashed_messages.pop_front());

    let Some(message) = message else {
      return Ok(0);
    };

    let mut pending = VecDeque::new();
    pending.push_back(message);

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &pending) {
      self.state.with_write(|state| {
        if let Some(message) = pending.pop_front() {
          state.stashed_messages.push_front(message);
        }
      });
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(1)
  }

  /// Re-enqueues all stashed user messages back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when mailbox enqueue fails. Remaining messages stay stashed.
  pub(crate) fn unstash_messages(&self) -> Result<usize, ActorError> {
    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let pending = self.state.with_write(|state| mem::take(&mut state.stashed_messages));

    if pending.is_empty() {
      return Ok(0);
    }

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &pending) {
      self.state.with_write(|state| state.stashed_messages = pending);
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(pending.len())
  }

  /// Re-enqueues up to `limit` stashed messages after applying `wrap`.
  ///
  /// # Errors
  ///
  /// Returns an error when message conversion or mailbox enqueue fails.
  pub(crate) fn unstash_messages_with_limit<F>(&self, limit: usize, mut wrap: F) -> Result<usize, ActorError>
  where
    F: FnMut(AnyMessage) -> Result<AnyMessage, ActorError>, {
    if limit == 0 {
      return Ok(0);
    }

    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let original_messages = self.state.with_write(|state| {
      let take_count = limit.min(state.stashed_messages.len());
      let mut messages = VecDeque::with_capacity(take_count);
      for _ in 0..take_count {
        if let Some(message) = state.stashed_messages.pop_front() {
          messages.push_back(message);
        }
      }
      messages
    });

    if original_messages.is_empty() {
      return Ok(0);
    }

    let mut wrapped_messages = VecDeque::with_capacity(original_messages.len());
    for message in original_messages.iter().cloned() {
      match wrap(message) {
        | Ok(wrapped) => wrapped_messages.push_back(wrapped),
        | Err(error) => {
          self.restore_stashed_messages(original_messages);
          return Err(error);
        },
      }
    }

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &wrapped_messages) {
      self.restore_stashed_messages(original_messages);
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(wrapped_messages.len())
  }

  fn restore_stashed_messages(&self, mut messages: VecDeque<AnyMessage>) {
    self.state.with_write(|state| {
      while let Some(message) = messages.pop_back() {
        state.stashed_messages.push_front(message);
      }
    });
  }

  /// Allocates and tracks a new adapter handle for message adapters.
  pub(crate) fn acquire_adapter_handle(&self) -> (AdapterRefHandleId, ArcShared<AdapterLifecycleState>) {
    self.state.with_write(|state| {
      let id = state.adapter_handle_counter.wrapping_add(1);
      state.adapter_handle_counter = id;
      let handle_id = id;
      let lifecycle = ArcShared::new(AdapterLifecycleState::new());
      let handle = AdapterRefHandle::new(handle_id, lifecycle.clone());
      state.adapter_handles.push(handle);
      (handle_id, lifecycle)
    })
  }

  /// Removes the specified adapter handle and marks it as stopped.
  pub(crate) fn remove_adapter_handle(&self, handle_id: AdapterRefHandleId) {
    self.state.with_write(|state| {
      let handles = &mut state.adapter_handles;
      if let Some(index) = handles.iter().position(|handle| handle.id() == handle_id) {
        let handle = handles.remove(index);
        handle.stop();
      }
    });
  }

  /// Drops every tracked adapter handle, notifying senders that the actor stopped.
  pub(crate) fn drop_adapter_refs(&self) {
    self.state.with_write(|state| {
      for handle in state.adapter_handles.iter() {
        handle.stop();
      }
      state.adapter_handles.clear();
    });
  }

  /// Registers a new pipe task targeting the actor itself and schedules its first poll.
  pub(crate) fn spawn_pipe_task(&self, future: ContextPipeFuture) -> Result<(), PipeSpawnError> {
    self.spawn_pipe_task_inner(future, None)
  }

  /// Registers a new pipe task targeting an external actor and schedules its first poll.
  pub(crate) fn spawn_pipe_to_task(&self, future: ContextPipeFuture, target: ActorRef) -> Result<(), PipeSpawnError> {
    self.spawn_pipe_task_inner(future, Some(target))
  }

  fn spawn_pipe_task_inner(&self, future: ContextPipeFuture, target: Option<ActorRef>) -> Result<(), PipeSpawnError> {
    if self.is_terminated() {
      return Err(PipeSpawnError::TargetStopped);
    }
    let id = self.state.with_write(|state| {
      if self.is_terminated() {
        return Err(PipeSpawnError::TargetStopped);
      }
      let id = ContextPipeTaskId::new(state.pipe_task_counter.wrapping_add(1));
      state.pipe_task_counter = id.get();
      let task = match target {
        | Some(t) => ContextPipeTask::new_with_target(id, future, self.pid, self.system(), t),
        | None => ContextPipeTask::new(id, future, self.pid, self.system()),
      };
      state.pipe_tasks.push(task);
      Ok(id)
    })?;
    self.poll_pipe_task(id);
    Ok(())
  }

  fn poll_pipe_task(&self, task_id: ContextPipeTaskId) {
    let result = self.state.with_write(|state| {
      let tasks = &mut state.pipe_tasks;
      let index = tasks.iter().position(|task| task.id() == task_id)?;
      match tasks[index].poll() {
        | Poll::Ready(message) => {
          let mut task = tasks.swap_remove(index);
          Some((message, task.take_delivery_target()))
        },
        | Poll::Pending => None,
      }
    });

    if let Some((Some(message), target)) = result {
      if let Some(mut target_ref) = target {
        let target_pid = target_ref.pid();
        if let Err(send_error) = target_ref.try_tell(message) {
          self.system().record_send_error(Some(target_pid), &send_error);
          self.system().emit_log(
            LogLevel::Warn,
            format!("pipe_to delivery failed for target {:?}: {:?}", target_pid, send_error),
            Some(self.pid()),
            None,
          );
        }
      } else {
        let self_pid = self.pid();
        let mut self_ref = self.actor_ref();
        if let Err(send_error) = self_ref.try_tell(message) {
          self.system().record_send_error(Some(self_pid), &send_error);
          self.system().emit_log(
            LogLevel::Warn,
            format!("pipe_to_self delivery failed for {:?}: {:?}", self_pid, send_error),
            Some(self_pid),
            None,
          );
        }
      }
    }
  }

  fn drop_pipe_tasks(&self) {
    self.state.with_write(|state| state.pipe_tasks.clear());
  }

  fn drop_stash_messages(&self) {
    self.state.with_write(|state| state.stashed_messages.clear());
  }

  fn drop_timer_handles(&self) {
    self.cancel_all_timers();
  }

  fn drop_watch_with_messages(&self) {
    self.state.with_write(|state| state.watch_with_messages.clear());
  }

  fn handle_pipe_task_ready(&self, task_id: ContextPipeTaskId) {
    self.poll_pipe_task(task_id)
  }

  fn notify_watchers_on_stop(&self) {
    let Some(recipients) = self.state.with_write(|state| {
      if state.watchers.is_empty() {
        return None;
      }
      Some(mem::take(&mut state.watchers))
    }) else {
      return;
    };

    for watcher in recipients {
      if let Err(send_error) = self.system().send_system_message(watcher, SystemMessage::Terminated(self.pid)) {
        self.system().record_send_error(Some(watcher), &send_error);
      }
    }
  }

  /// Delivers a termination notification for the given pid.
  ///
  /// When a custom message was registered via [`register_watch_with`](Self::register_watch_with),
  /// the message is enqueued into the actor mailbox (delivered asynchronously on a later turn).
  /// Otherwise, [`Actor::on_terminated`] is invoked synchronously within this call.
  pub(crate) fn handle_terminated(&self, terminated_pid: Pid) -> Result<(), ActorError> {
    let custom_message = self.take_watch_with_message(terminated_pid);
    if let Some(message) = custom_message {
      self.actor_ref().try_tell(message).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(())
    } else {
      let mut ctx = self.make_context();
      let result = self.actor.with_write(|actor| actor.on_terminated(&mut ctx, terminated_pid));
      ctx.clear_sender();
      result
    }
  }

  /// Registers a custom message to deliver when the watched target terminates.
  pub(crate) fn register_watch_with(&self, target: Pid, message: AnyMessage) {
    self.state.with_write(|state| {
      state.watch_with_messages.retain(|(pid, _)| *pid != target);
      state.watch_with_messages.push((target, message));
    });
  }

  /// Removes any custom watch-with message for the given target.
  pub(crate) fn remove_watch_with(&self, target: Pid) {
    self.state.with_write(|state| state.watch_with_messages.retain(|(pid, _)| *pid != target));
  }

  fn take_watch_with_message(&self, target: Pid) -> Option<AnyMessage> {
    self.state.with_write(|state| {
      if let Some(index) = state.watch_with_messages.iter().position(|(pid, _)| *pid == target) {
        let (_, message) = state.watch_with_messages.swap_remove(index);
        Some(message)
      } else {
        None
      }
    })
  }

  fn handle_create(&self) -> Result<(), ActorError> {
    let outcome = self.run_pre_start(LifecycleStage::Started);
    if let Err(ref error) = outcome {
      self.report_failure(error, None);
    }
    outcome
  }

  fn handle_recreate(&self) -> Result<(), ActorError> {
    {
      let mut ctx = self.make_context();
      ctx.cancel_receive_timeout();
      self.actor.with_write(|actor| actor.pre_restart(&mut ctx))?;
      ctx.clear_sender();
    }

    self.drop_pipe_tasks();
    self.drop_stash_messages();
    self.drop_timer_handles();
    self.drop_watch_with_messages();
    self.publish_lifecycle(LifecycleStage::Stopped);
    self.recreate_actor();
    let outcome = self.run_pre_start(LifecycleStage::Restarted);
    if outcome.is_ok() {
      self.mailbox().resume();
    }
    outcome
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn watchers_snapshot(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.watchers.clone())
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let mut ctx = self.make_context();
    ctx.cancel_receive_timeout();
    let result = self.actor.with_write(|actor| actor.post_stop(&mut ctx));
    ctx.clear_sender();
    if result.is_ok() {
      self.publish_lifecycle(LifecycleStage::Stopped);
    }

    let children_snapshot = self.children();
    for child in &children_snapshot {
      if let Err(send_error) = self.system().send_system_message(*child, SystemMessage::Stop) {
        self.system().record_send_error(Some(*child), &send_error);
      }
    }

    self.clear_child_stats(&children_snapshot);
    self.drop_stash_messages();
    self.drop_timer_handles();
    self.mark_terminated();
    self.notify_watchers_on_stop();

    if let Some(parent) = self.parent {
      self.system().unregister_child(Some(parent), self.pid);
    }

    self.system().release_name(self.parent, &self.name);
    self.system().remove_cell(&self.pid);

    if let Some(kind) = self.system().guardian_kind_by_pid(self.pid) {
      self.system().mark_guardian_stopped(kind);
      match kind {
        | GuardianKind::Root => {
          self.system().mark_terminated();
        },
        | GuardianKind::User | GuardianKind::System => {
          if !self.system().guardian_alive(GuardianKind::Root) {
            self.system().mark_terminated();
          }
        },
      }
    }

    result
  }

  fn handle_kill(&self, snapshot: Option<FailureMessageSnapshot>) -> Result<(), ActorError> {
    let error = ActorError::fatal("Kill");
    self.report_failure(&error, snapshot);
    Err(error)
  }

  fn report_failure(&self, error: &ActorError, snapshot: Option<FailureMessageSnapshot>) {
    self.mailbox().suspend();
    let timestamp = self.system().monotonic_now();
    let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
    self.system().report_failure(payload);
  }

  fn handle_failure_message(&self, payload: &FailurePayload) {
    let actor_error = payload.to_actor_error();
    let now = self.system().monotonic_now();
    let payload_ref = &payload;
    let (directive, affected) = self.handle_child_failure(payload.child(), &actor_error, now);

    {
      let mut ctx = self.make_context();
      if let Err(ref error) =
        self.actor.with_write(|actor| actor.on_child_failed(&mut ctx, payload.child(), &actor_error))
      {
        self.report_failure(error, None);
      }
      ctx.clear_sender();
    }

    match directive {
      | SupervisorDirective::Restart => {
        let mut restart_failed = false;
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Recreate) {
            self.system().record_send_error(Some(target), &send_error);
            restart_failed = true;
          }
        }

        if restart_failed {
          self.system().record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
          let snapshot = payload.message().cloned();
          let escalated = FailurePayload::from_error(self.pid, &actor_error, snapshot, self.system().monotonic_now());
          self.system().report_failure(escalated);
        } else {
          self.system().record_failure_outcome(payload.child(), FailureOutcome::Restart, payload_ref);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Stop) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Stop) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
        let snapshot = payload.message().cloned();
        let escalated = FailurePayload::from_error(self.pid, &actor_error, snapshot, self.system().monotonic_now());
        self.system().report_failure(escalated);
      },
      | SupervisorDirective::Resume => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Resume) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Resume, payload_ref);
      },
    }
  }

  fn run_pre_start(&self, stage: LifecycleStage) -> Result<(), ActorError> {
    let mut ctx = self.make_context();
    let outcome = self.actor.with_write(|actor| actor.pre_start(&mut ctx));
    ctx.clear_sender();
    if outcome.is_ok() {
      self.publish_lifecycle(stage);
    }
    outcome
  }

  fn publish_lifecycle(&self, stage: LifecycleStage) {
    let timestamp = self.system().monotonic_now();
    let event = LifecycleEvent::new(self.pid, self.parent, self.name.clone(), stage, timestamp);
    self.system().publish_event(&EventStreamEvent::Lifecycle(event));
  }
}

/// Internal invoker that bridges dispatcher message delivery to actor cell.
///
/// Uses a weak reference to avoid circular reference between ActorCell and DispatcherCore.
struct ActorCellInvoker {
  cell: WeakShared<ActorCell>,
}

impl ActorCellInvoker {
  /// Upgrades the weak cell reference to a strong reference.
  ///
  /// Returns `None` if the actor cell has been dropped.
  fn cell(&self) -> Option<ArcShared<ActorCell>> {
    self.cell.upgrade()
  }
}

impl MessageInvoker for ActorCellInvoker {
  fn invoke_user_message(&mut self, message: AnyMessage) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the message
      return Ok(());
    };
    if cell.is_terminated() {
      return Ok(());
    }
    if let Some(system_message) = message.payload().downcast_ref::<SystemMessage>() {
      match system_message {
        | SystemMessage::PoisonPill => return cell.handle_stop(),
        | SystemMessage::Kill => {
          let snapshot = FailureMessageSnapshot::from_message(&message);
          return cell.handle_kill(Some(snapshot));
        },
        | _ => {},
      }
    }
    if let Some(identify) = message.payload().downcast_ref::<Identify>() {
      if let Some(mut sender) = message.sender().cloned() {
        let identity = ActorIdentity::found(identify.correlation_id().clone(), cell.actor_ref());
        // Best-effort reply: the requester may have stopped before the reply arrives.
        sender.try_tell(AnyMessage::new(identity)).map_err(|error| ActorError::from_send_error(&error))?;
      }
      // NOTE: No reply is sent if sender is None (no deadLetters in no_std).
      // Use with_sender() to receive ActorIdentity replies.
      return Ok(());
    }
    let mut ctx = cell.make_context();
    let failure_candidate = message.clone();
    let result = cell.actor.with_write(|actor| cell.pipeline.invoke_user(&mut **actor, &mut ctx, message));
    match &result {
      | Ok(()) => ctx.reschedule_receive_timeout(),
      | Err(error) => {
        let snapshot = FailureMessageSnapshot::from_message(&failure_candidate);
        cell.report_failure(error, Some(snapshot));
      },
    }
    result
  }

  fn invoke_system_message(&mut self, message: SystemMessage) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the message
      return Ok(());
    };
    if cell.is_terminated() {
      return Ok(());
    }
    match message {
      | SystemMessage::PoisonPill => cell.handle_stop(),
      | SystemMessage::Kill => {
        let payload: ArcShared<dyn core::any::Any + Send + Sync + 'static> = ArcShared::new(SystemMessage::Kill);
        let snapshot = FailureMessageSnapshot::new(payload, None);
        cell.handle_kill(Some(snapshot))
      },
      | SystemMessage::Stop => cell.handle_stop(),
      | SystemMessage::Create => cell.handle_create(),
      | SystemMessage::Recreate => cell.handle_recreate(),
      | SystemMessage::Failure(ref payload) => {
        cell.handle_failure_message(payload);
        Ok(())
      },
      | SystemMessage::Suspend => {
        cell.mailbox().suspend();
        Ok(())
      },
      | SystemMessage::Resume => {
        cell.mailbox().resume();
        Ok(())
      },
      | SystemMessage::Watch(pid) => {
        cell.handle_watch(pid);
        Ok(())
      },
      | SystemMessage::Unwatch(pid) => {
        cell.handle_unwatch(pid);
        Ok(())
      },
      | SystemMessage::StopChild(pid) => {
        cell.stop_child(pid);
        Ok(())
      },
      | SystemMessage::Terminated(pid) => cell.handle_terminated(pid),
      | SystemMessage::PipeTask(task_id) => {
        cell.handle_pipe_task_ready(task_id);
        Ok(())
      },
    }
  }

  fn invoke_mailbox_pressure(&mut self, event: &MailboxPressureEvent) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the notification
      return Ok(());
    };
    let mut ctx = cell.make_context();
    let result = cell.actor.with_write(|actor| actor.on_mailbox_pressure(&mut ctx, event));
    if let Err(ref error) = result {
      cell.report_failure(error, None);
    }
    result
  }
}

impl ActorCell {
  pub(crate) fn handle_child_failure(
    &self,
    child: Pid,
    error: &ActorError,
    now: Duration,
  ) -> (SupervisorDirective, Vec<Pid>) {
    // Get supervisor strategy dynamically from actor instance
    let strategy = {
      let mut ctx = self.make_context();
      self.actor.with_read(|actor| actor.supervisor_strategy(&mut ctx))
    };

    let directive = {
      self.state.with_write(|state| {
        let entry = find_or_insert_stats(&mut state.child_stats, child);
        strategy.handle_failure(entry, error, now)
      })
    };

    let affected = match strategy.kind() {
      | SupervisorStrategyKind::OneForOne => vec![child],
      | SupervisorStrategyKind::AllForOne => self.state.with_read(|state| state.children.clone()),
    };

    if matches!(directive, SupervisorDirective::Stop) {
      self.clear_child_stats(&affected);
    }

    (directive, affected)
  }

  fn clear_child_stats(&self, children: &[Pid]) {
    if children.is_empty() {
      return;
    }
    self.state.with_write(|state| state.child_stats.retain(|(pid, _)| !children.contains(pid)));
  }
}

fn find_or_insert_stats(entries: &mut Vec<(Pid, RestartStatistics)>, pid: Pid) -> &mut RestartStatistics {
  if let Some(index) = entries.iter().position(|(child, _)| *child == pid) {
    return &mut entries[index].1;
  }
  let new_index = entries.len();
  entries.push((pid, RestartStatistics::new()));
  &mut entries[new_index].1
}
