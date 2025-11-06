//! Shared, mutable state owned by the actor system.

#[cfg(test)]
mod tests;

use alloc::{format, string::String, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use hashbrown::HashMap;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
  actor_prim::{ActorCellGeneric, Pid},
  dead_letter::{DeadLetterEntryGeneric, DeadLetterGeneric},
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamGeneric},
  futures::ActorFuture,
  logging::{LogEvent, LogLevel},
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  spawn::{NameRegistry, NameRegistryError, SpawnError},
  supervision::SupervisorDirective,
};

mod failure_outcome;

pub use failure_outcome::FailureOutcome;

/// Type alias for ask future collections.
type AskFutureVec<TB> = Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>>;

/// Captures global actor system state.
pub struct SystemStateGeneric<TB: RuntimeToolbox + 'static> {
  next_pid:               AtomicU64,
  clock:                  AtomicU64,
  cells:                  ToolboxMutex<HashMap<Pid, ArcShared<ActorCellGeneric<TB>>>, TB>,
  registries:             ToolboxMutex<HashMap<Option<Pid>, NameRegistry>, TB>,
  user_guardian:          ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>, TB>,
  ask_futures:            ToolboxMutex<AskFutureVec<TB>, TB>,
  termination:            ArcShared<ActorFuture<(), TB>>,
  terminated:             AtomicBool,
  event_stream:           ArcShared<EventStreamGeneric<TB>>,
  dead_letter:            ArcShared<DeadLetterGeneric<TB>>,
  failure_total:          AtomicU64,
  failure_restart_total:  AtomicU64,
  failure_stop_total:     AtomicU64,
  failure_escalate_total: AtomicU64,
  failure_inflight:       AtomicU64,
}

/// Type alias for [SystemStateGeneric] with the default [NoStdToolbox].
pub type SystemState = SystemStateGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SystemStateGeneric<TB> {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    const DEAD_LETTER_CAPACITY: usize = 512;
    let event_stream = ArcShared::new(EventStreamGeneric::default());
    let dead_letter = ArcShared::new(DeadLetterGeneric::new(event_stream.clone(), DEAD_LETTER_CAPACITY));
    Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      cells: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      registries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      user_guardian: <TB::MutexFamily as SyncMutexFamily>::create(None),
      ask_futures: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      termination: ArcShared::new(ActorFuture::<(), TB>::new()),
      terminated: AtomicBool::new(false),
      event_stream,
      dead_letter,
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
    }
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Registers the provided actor cell in the global registry.
  pub(crate) fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.cells.lock().insert(cell.pid(), cell);
  }

  /// Removes the actor cell associated with the pid.
  pub(crate) fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.cells.lock().remove(pid)
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.cells.lock().get(pid).cloned()
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns an error if the requested name is already taken.
  pub(crate) fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    let mut registries = self.registries.lock();
    let registry = registries.entry(parent).or_insert_with(NameRegistry::new);

    match hint {
      | Some(name) => {
        registry.register(name, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
        Ok(String::from(name))
      },
      | None => {
        let generated = registry.generate_anonymous(pid);
        registry.register(&generated, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
        Ok(generated)
      },
    }
  }

  /// Releases the association between a name and its pid in the registry.
  pub(crate) fn release_name(&self, parent: Option<Pid>, name: &str) {
    if let Some(registry) = self.registries.lock().get_mut(&parent) {
      registry.remove(name);
    }
  }

  /// Stores the user guardian cell reference.
  pub(crate) fn set_user_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    *self.user_guardian.lock() = Some(cell);
  }

  /// Clears the guardian if the provided pid matches.
  pub(crate) fn clear_guardian(&self, pid: Pid) -> bool {
    let mut guardian = self.user_guardian.lock();
    if guardian.as_ref().map(|cell| cell.pid()) == Some(pid) {
      *guardian = None;
      return true;
    }
    false
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.user_guardian.lock().clone()
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.user_guardian.lock().as_ref().map(|cell| cell.pid())
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.event_stream.clone()
  }

  /// Returns a snapshot of deadletter entries.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.dead_letter.entries()
  }

  /// Registers an ask future so the actor system can track its completion.
  pub(crate) fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) {
    self.ask_futures.lock().push(future);
  }

  /// Publishes an event to all event stream subscribers.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.event_stream.publish(event);
  }

  /// Emits a log event via the event stream.
  pub fn emit_log(&self, level: LogLevel, message: String, origin: Option<Pid>) {
    let timestamp = self.monotonic_now();
    let event = LogEvent::new(level, message, timestamp, origin);
    self.event_stream.publish(&EventStreamEvent::Log(event));
  }

  /// Registers a child under the specified parent pid.
  pub(crate) fn register_child(&self, parent: Pid, child: Pid) {
    if let Some(cell) = self.cell(&parent) {
      cell.register_child(child);
    }
  }

  /// Removes a child from its parent's supervision registry.
  pub(crate) fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    if let Some(parent_pid) = parent
      && let Some(cell) = self.cell(&parent_pid)
    {
      cell.unregister_child(&child);
    }
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub(crate) fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.cell(&parent).map_or_else(Vec::new, |cell| cell.children())
  }

  /// Sends a system message to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor doesn't exist or the message cannot be enqueued.
  pub(crate) fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    if let Some(cell) = self.cell(&pid) {
      cell.dispatcher().enqueue_system(message)
    } else {
      match message {
        | SystemMessage::Watch(watcher) => {
          let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
          Ok(())
        },
        | SystemMessage::Unwatch(_) => Ok(()),
        | SystemMessage::Terminated(_) => Ok(()),
        | other => Err(SendError::<TB>::closed(AnyMessageGeneric::new(other))),
      }
    }
  }

  /// Records a send error for diagnostics.
  pub(crate) fn record_send_error(&self, recipient: Option<Pid>, error: &SendError<TB>) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_send_error(recipient, error, timestamp);
  }

  /// Marks the system as terminated and completes the termination future.
  pub(crate) fn mark_terminated(&self) {
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    self.termination.complete(());
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub(crate) fn termination_future(&self) -> ArcShared<ActorFuture<(), TB>> {
    self.termination.clone()
  }

  /// Drains ask futures that have completed since the previous inspection.
  pub(crate) fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> {
    let mut registry = self.ask_futures.lock();
    let mut ready = Vec::new();
    let mut index = 0_usize;

    while index < registry.len() {
      if registry[index].is_ready() {
        ready.push(registry.swap_remove(index));
      } else {
        index += 1;
      }
    }

    ready
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    let ticks = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    Duration::from_millis(ticks)
  }

  /// Records a failure and routes it to the supervising hierarchy.
  pub(crate) fn report_failure(&self, mut payload: FailurePayload) {
    self.failure_total.fetch_add(1, Ordering::Relaxed);
    self.failure_inflight.fetch_add(1, Ordering::AcqRel);
    let message = format!("actor {:?} failed: {}", payload.child(), payload.reason().as_str());
    self.emit_log(LogLevel::Error, message, Some(payload.child()));

    if let Some(parent_pid) = self.parent_of(&payload.child())
      && let Some(parent_cell) = self.cell(&parent_pid)
    {
      if let Some(stats) = parent_cell.snapshot_child_restart_stats(payload.child()) {
        payload = payload.with_restart_stats(stats);
      }
      if self.send_system_message(parent_pid, SystemMessage::Failure(payload.clone())).is_ok() {
        return;
      }
      let payload_ref = &payload;
      self.record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
      self.stop_actor(payload.child());
      return;
    }

    let payload_ref = &payload;
    self.record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
    self.stop_actor(payload.child());
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub(crate) fn record_failure_outcome(&self, child: Pid, outcome: FailureOutcome, payload: &FailurePayload) {
    self.failure_inflight.fetch_sub(1, Ordering::AcqRel);
    let counter = match outcome {
      | FailureOutcome::Restart => &self.failure_restart_total,
      | FailureOutcome::Stop => &self.failure_stop_total,
      | FailureOutcome::Escalate => &self.failure_escalate_total,
    };
    counter.fetch_add(1, Ordering::Relaxed);
    let label = match outcome {
      | FailureOutcome::Restart => "restart",
      | FailureOutcome::Stop => "stop",
      | FailureOutcome::Escalate => "escalate",
    };
    let message = format!("failure outcome {} for {:?} (reason: {})", label, child, payload.reason().as_str());
    self.emit_log(LogLevel::Info, message, Some(child));
  }

  #[allow(dead_code)]
  fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    let Some(parent_pid) = parent else {
      self.stop_actor(pid);
      return;
    };

    let Some(parent_cell) = self.cell(&parent_pid) else {
      self.stop_actor(pid);
      return;
    };

    let parent_cell_ref = &*parent_cell;
    let parent_parent = parent_cell_ref.parent();
    let now = self.monotonic_now();
    let (directive, affected) = parent_cell_ref.handle_child_failure(pid, error, now);

    match directive {
      | SupervisorDirective::Restart => {
        let mut escalate_due_to_recreate_failure = false;
        for target in affected {
          if let Err(send_error) = self.send_system_message(target, SystemMessage::Recreate) {
            self.record_send_error(Some(target), &send_error);
            let _ = self.send_system_message(target, SystemMessage::Stop);
            escalate_due_to_recreate_failure = true;
          }
        }
        if escalate_due_to_recreate_failure {
          self.handle_failure(parent_pid, parent_parent, error);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          self.stop_actor(target);
        }
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          self.stop_actor(target);
        }
        self.handle_failure(parent_pid, parent_parent, error);
      },
    }
  }

  fn stop_actor(&self, pid: Pid) {
    let _ = self.send_system_message(pid, SystemMessage::Stop);
  }

  fn parent_of(&self, pid: &Pid) -> Option<Pid> {
    self.cell(pid).and_then(|cell| cell.parent())
  }
}

impl<TB: RuntimeToolbox + 'static> Default for SystemStateGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for SystemStateGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for SystemStateGeneric<TB> {}
