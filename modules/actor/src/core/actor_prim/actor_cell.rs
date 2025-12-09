//! Runtime container responsible for executing an actor instance.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::{mem, task::Poll, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};
use portable_atomic::{AtomicBool, Ordering};

use crate::core::{
  actor_prim::{
    Actor, ActorContextGeneric, ActorSharedGeneric, ContextPipeTaskId, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSenderSharedGeneric},
    context_pipe_task::{ContextPipeFuture, ContextPipeTask},
    pipe_spawn_error::PipeSpawnError,
  },
  dispatcher::DispatcherGeneric,
  error::ActorError,
  event_stream::EventStreamEvent,
  lifecycle::{LifecycleEvent, LifecycleStage},
  mailbox::{BackpressurePublisherGeneric, MailboxCapacity, MailboxGeneric, MailboxInstrumentationGeneric},
  messaging::{
    AnyMessageGeneric, FailureMessageSnapshot, FailurePayload, SystemMessage,
    message_invoker::{MessageInvoker, MessageInvokerPipelineGeneric, MessageInvokerShared},
  },
  props::{ActorFactorySharedGeneric, PropsGeneric},
  spawn::SpawnError,
  supervision::{RestartStatistics, SupervisorDirective, SupervisorStrategyKind},
  system::{ActorSystemGeneric, FailureOutcome, GuardianKind, SystemStateSharedGeneric},
  typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
};

struct ActorCellState<TB: RuntimeToolbox + 'static> {
  children:               Vec<Pid>,
  child_stats:            Vec<(Pid, RestartStatistics)>,
  watchers:               Vec<Pid>,
  pipe_tasks:             Vec<ContextPipeTask<TB>>,
  adapter_handles:        Vec<AdapterRefHandle<TB>>,
  adapter_handle_counter: u64,
  pipe_task_counter:      u64,
}

impl<TB: RuntimeToolbox + 'static> ActorCellState<TB> {
  const fn new() -> Self {
    Self {
      children:               Vec::new(),
      child_stats:            Vec::new(),
      watchers:               Vec::new(),
      pipe_tasks:             Vec::new(),
      adapter_handles:        Vec::new(),
      adapter_handle_counter: 0,
      pipe_task_counter:      0,
    }
  }
}

/// Runtime container responsible for executing an actor instance.
pub struct ActorCellGeneric<TB: RuntimeToolbox + 'static> {
  pid:        Pid,
  parent:     Option<Pid>,
  name:       String,
  system:     SystemStateSharedGeneric<TB>,
  factory:    ActorFactorySharedGeneric<TB>,
  actor:      ActorSharedGeneric<TB>,
  pipeline:   MessageInvokerPipelineGeneric<TB>,
  mailbox:    ArcShared<MailboxGeneric<TB>>,
  dispatcher: DispatcherGeneric<TB>,
  sender:     ActorRefSenderSharedGeneric<TB>,
  state:      ToolboxMutex<ActorCellState<TB>, TB>,
  terminated: AtomicBool,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorCellGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorCellGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> ActorCellGeneric<TB> {
  /// Creates a new actor cell using the provided runtime state and props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox configuration is incompatible
  /// with the dispatcher executor (e.g., using Block strategy with a non-blocking executor).
  pub fn create(
    system: SystemStateSharedGeneric<TB>,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &PropsGeneric<TB>,
  ) -> Result<ArcShared<Self>, SpawnError> {
    let mailbox = ArcShared::new(MailboxGeneric::new(props.mailbox_policy()));
    {
      let mailbox_config = props.mailbox();
      let policy = mailbox_config.policy();
      let capacity = match policy.capacity() {
        | MailboxCapacity::Bounded { capacity } => Some(capacity.get()),
        | MailboxCapacity::Unbounded => None,
      };
      let throughput = policy.throughput_limit().map(|limit| limit.get());
      let warn_threshold = mailbox_config.warn_threshold().map(|threshold| threshold.get());
      let instrumentation =
        MailboxInstrumentationGeneric::new(system.clone(), pid, capacity, throughput, warn_threshold);
      mailbox.set_instrumentation(instrumentation);
    }
    let dispatcher = props.dispatcher().build_dispatcher(mailbox.clone())?;
    mailbox.attach_backpressure_publisher(BackpressurePublisherGeneric::from_dispatcher(dispatcher.clone()));
    let sender = dispatcher.into_sender();
    let factory = props.factory().clone();
    let actor = ActorSharedGeneric::new(factory.with_write(|f| f.create()));
    let state = <TB::MutexFamily as SyncMutexFamily>::create(ActorCellState::new());

    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      system,
      factory,
      actor,
      pipeline: MessageInvokerPipelineGeneric::new(),
      mailbox,
      dispatcher,
      sender,
      state,
      terminated: AtomicBool::new(false),
    });

    {
      // Dispatcher keeps a shared reference to the invoker for message delivery.
      let invoker: MessageInvokerShared<TB> =
        MessageInvokerShared::new(Box::new(ActorCellInvoker { cell: cell.clone() }));
      cell.dispatcher.register_invoker(invoker);
    }

    Ok(cell)
  }

  /// Recreates the actor instance from the stored factory.
  fn recreate_actor(&self) {
    self.actor.with_write(|actor| {
      *actor = self.factory.with_write(|f| f.create());
    });
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

  /// Returns a handle to the mailbox managed by this cell.
  #[must_use]
  pub fn mailbox(&self) -> ArcShared<MailboxGeneric<TB>> {
    self.mailbox.clone()
  }

  /// Returns the dispatcher associated with this cell.
  #[must_use]
  pub fn dispatcher(&self) -> DispatcherGeneric<TB> {
    self.dispatcher.clone()
  }

  /// Returns a sender handle targeting this actor cell's mailbox.
  #[must_use]
  pub(crate) fn mailbox_sender(&self) -> ActorRefSenderSharedGeneric<TB> {
    self.sender.clone()
  }

  /// Produces an actor reference targeting this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRefGeneric<TB> {
    ActorRefGeneric::from_shared(self.pid, self.sender.clone(), self.system.clone())
  }

  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    let mut state = self.state.lock();
    if !state.children.contains(&pid) {
      state.children.push(pid);
    }
    find_or_insert_stats(&mut state.child_stats, pid);
  }

  /// Removes a child pid from supervision tracking.
  pub fn unregister_child(&self, pid: &Pid) {
    let mut state = self.state.lock();
    state.children.retain(|child| child != pid);
    state.child_stats.retain(|(child, _)| child != pid);
  }

  fn stop_child(&self, pid: Pid) {
    let should_stop = { self.state.lock().children.contains(&pid) };
    if should_stop {
      let _ = self.system.send_system_message(pid, SystemMessage::Stop);
    }
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.state.lock().children.clone()
  }

  pub(crate) fn snapshot_child_restart_stats(&self, pid: Pid) -> Option<RestartStatistics> {
    let state = self.state.lock();
    state.child_stats.iter().find(|(child, _)| *child == pid).map(|(_, record)| record.clone())
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
      let _ = self.system.send_system_message(watcher, SystemMessage::Terminated(self.pid));
      return;
    }

    let mut state = self.state.lock();
    if !state.watchers.contains(&watcher) {
      state.watchers.push(watcher);
    }
  }

  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.state.lock().watchers.retain(|pid| *pid != watcher);
  }

  /// Allocates and tracks a new adapter handle for message adapters.
  pub(crate) fn acquire_adapter_handle(&self) -> (AdapterRefHandleId, ArcShared<AdapterLifecycleState<TB>>) {
    let mut state = self.state.lock();
    let id = state.adapter_handle_counter.wrapping_add(1);
    state.adapter_handle_counter = id;
    let handle_id = AdapterRefHandleId::new(id);
    let lifecycle = ArcShared::new(AdapterLifecycleState::new(self.system.clone(), self.pid));
    let handle = AdapterRefHandle::new(handle_id, lifecycle.clone());
    state.adapter_handles.push(handle);
    (handle_id, lifecycle)
  }

  /// Removes the specified adapter handle and marks it as stopped.
  pub(crate) fn remove_adapter_handle(&self, handle_id: AdapterRefHandleId) {
    let mut state = self.state.lock();
    let handles = &mut state.adapter_handles;
    if let Some(index) = handles.iter().position(|handle| handle.id() == handle_id) {
      let handle = handles.remove(index);
      handle.stop();
    }
  }

  /// Drops every tracked adapter handle, notifying senders that the actor stopped.
  pub(crate) fn drop_adapter_refs(&self) {
    let mut state = self.state.lock();
    for handle in state.adapter_handles.iter() {
      handle.stop();
    }
    state.adapter_handles.clear();
  }

  /// Registers a new pipe task and schedules its first poll.
  pub(crate) fn spawn_pipe_task(&self, future: ContextPipeFuture<TB>) -> Result<(), PipeSpawnError> {
    if self.is_terminated() {
      return Err(PipeSpawnError::TargetStopped);
    }
    let mut state = self.state.lock();
    let id = ContextPipeTaskId::new(state.pipe_task_counter.wrapping_add(1));
    state.pipe_task_counter = id.get();
    let task = ContextPipeTask::new(id, future, self.pid, self.system.clone());
    state.pipe_tasks.push(task);
    drop(state);
    self.poll_pipe_task(id);
    Ok(())
  }

  fn poll_pipe_task(&self, task_id: ContextPipeTaskId) {
    let message = {
      let mut state = self.state.lock();
      let tasks = &mut state.pipe_tasks;
      let Some(index) = tasks.iter().position(|task| task.id() == task_id) else {
        return;
      };
      match tasks[index].poll() {
        | Poll::Ready(message) => {
          tasks.swap_remove(index);
          Some(message)
        },
        | Poll::Pending => None,
      }
    };

    if let Some(message) = message {
      match self.actor_ref().tell(message) {
        | Ok(()) => {},
        | Err(error) => self.system.record_send_error(Some(self.pid), &error),
      }
    }
  }

  fn drop_pipe_tasks(&self) {
    self.state.lock().pipe_tasks.clear();
  }

  fn handle_pipe_task_ready(&self, task_id: ContextPipeTaskId) {
    self.poll_pipe_task(task_id);
  }

  fn notify_watchers_on_stop(&self) {
    let mut state = self.state.lock();
    if state.watchers.is_empty() {
      return;
    }
    let recipients = mem::take(&mut state.watchers);
    drop(state);

    for watcher in recipients {
      let _ = self.system.send_system_message(watcher, SystemMessage::Terminated(self.pid));
    }
  }

  pub(crate) fn handle_terminated(&self, terminated_pid: Pid) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContextGeneric::new(&system, self.pid);
    let result = self.actor.with_write(|actor| actor.on_terminated(&mut ctx, terminated_pid));
    ctx.clear_reply_to();
    result
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
      let system = ActorSystemGeneric::from_state(self.system.clone());
      let mut ctx = ActorContextGeneric::new(&system, self.pid);
      self.actor.with_write(|actor| actor.post_stop(&mut ctx))?;
      ctx.clear_reply_to();
    }

    self.drop_pipe_tasks();
    self.publish_lifecycle(LifecycleStage::Stopped);
    self.recreate_actor();
    let outcome = self.run_pre_start(LifecycleStage::Restarted);
    if outcome.is_ok() {
      self.mailbox.resume();
    }
    outcome
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn watchers_snapshot(&self) -> Vec<Pid> {
    self.state.lock().watchers.clone()
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContextGeneric::new(&system, self.pid);
    let result = self.actor.with_write(|actor| actor.post_stop(&mut ctx));
    ctx.clear_reply_to();
    if result.is_ok() {
      self.publish_lifecycle(LifecycleStage::Stopped);
    }

    let children_snapshot = self.children();
    for child in &children_snapshot {
      let _ = self.system.send_system_message(*child, SystemMessage::Stop);
    }

    self.clear_child_stats(&children_snapshot);
    self.mark_terminated();
    self.notify_watchers_on_stop();

    if let Some(parent) = self.parent {
      self.system.unregister_child(Some(parent), self.pid);
    }

    self.system.release_name(self.parent, &self.name);
    let _ = self.system.remove_cell(&self.pid);

    match self.system.clear_guardian(self.pid) {
      | Some(GuardianKind::Root) => {
        self.system.clone().mark_terminated();
      },
      | Some(GuardianKind::User) | Some(GuardianKind::System) => {
        if !self.system.guardian_alive(GuardianKind::Root) {
          self.system.clone().mark_terminated();
        }
      },
      | None => {},
    }

    result
  }

  fn report_failure(&self, error: &ActorError, snapshot: Option<FailureMessageSnapshot>) {
    self.mailbox.suspend();
    let timestamp = self.system.monotonic_now();
    let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
    self.system.report_failure(payload);
  }

  fn handle_failure_message(&self, payload: &FailurePayload) {
    let actor_error = payload.to_actor_error();
    let now = self.system.monotonic_now();
    let payload_ref = &payload;
    let (directive, affected) = self.handle_child_failure(payload.child(), &actor_error, now);

    match directive {
      | SupervisorDirective::Restart => {
        let mut restart_failed = false;
        for target in affected {
          if let Err(send_error) = self.system.send_system_message(target, SystemMessage::Recreate) {
            self.system.record_send_error(Some(target), &send_error);
            restart_failed = true;
          }
        }

        if restart_failed {
          self.system.record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
          let snapshot = payload.message().cloned();
          let escalated = FailurePayload::from_error(self.pid, &actor_error, snapshot, self.system.monotonic_now());
          self.system.report_failure(escalated);
        } else {
          self.system.record_failure_outcome(payload.child(), FailureOutcome::Restart, payload_ref);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          let _ = self.system.send_system_message(target, SystemMessage::Stop);
        }
        self.system.record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          let _ = self.system.send_system_message(target, SystemMessage::Stop);
        }
        self.system.record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
        let snapshot = payload.message().cloned();
        let escalated = FailurePayload::from_error(self.pid, &actor_error, snapshot, self.system.monotonic_now());
        self.system.report_failure(escalated);
      },
    }
  }

  fn run_pre_start(&self, stage: LifecycleStage) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContextGeneric::new(&system, self.pid);
    let outcome = self.actor.with_write(|actor| actor.pre_start(&mut ctx));
    ctx.clear_reply_to();
    if outcome.is_ok() {
      self.publish_lifecycle(stage);
    }
    outcome
  }

  fn publish_lifecycle(&self, stage: LifecycleStage) {
    let timestamp = self.system.monotonic_now();
    let event = LifecycleEvent::new(self.pid, self.parent, self.name.clone(), stage, timestamp);
    self.system.publish_event(&EventStreamEvent::Lifecycle(event));
  }
}

struct ActorCellInvoker<TB: RuntimeToolbox + 'static> {
  cell: ArcShared<ActorCellGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> MessageInvoker<TB> for ActorCellInvoker<TB> {
  fn invoke_user_message(&mut self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.cell.system.clone());
    let mut ctx = ActorContextGeneric::new(&system, self.cell.pid);
    let failure_candidate = message.clone();
    let result = self.cell.actor.with_write(|actor| self.cell.pipeline.invoke_user(&mut **actor, &mut ctx, message));
    if let Err(ref error) = result {
      let snapshot = FailureMessageSnapshot::from_message(&failure_candidate);
      self.cell.report_failure(error, Some(snapshot));
    }
    result
  }

  fn invoke_system_message(&mut self, message: SystemMessage) -> Result<(), ActorError> {
    match message {
      | SystemMessage::Stop => self.cell.handle_stop(),
      | SystemMessage::Create => self.cell.handle_create(),
      | SystemMessage::Recreate => self.cell.handle_recreate(),
      | SystemMessage::Failure(ref payload) => {
        self.cell.handle_failure_message(payload);
        Ok(())
      },
      | SystemMessage::Suspend => {
        self.cell.mailbox.suspend();
        Ok(())
      },
      | SystemMessage::Resume => {
        self.cell.mailbox.resume();
        Ok(())
      },
      | SystemMessage::Watch(pid) => {
        self.cell.handle_watch(pid);
        Ok(())
      },
      | SystemMessage::Unwatch(pid) => {
        self.cell.handle_unwatch(pid);
        Ok(())
      },
      | SystemMessage::StopChild(pid) => {
        self.cell.stop_child(pid);
        Ok(())
      },
      | SystemMessage::Terminated(pid) => self.cell.handle_terminated(pid),
      | SystemMessage::PipeTask(task_id) => {
        self.cell.handle_pipe_task_ready(task_id);
        Ok(())
      },
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorCellGeneric<TB> {
  pub(crate) fn handle_child_failure(
    &self,
    child: Pid,
    error: &ActorError,
    now: Duration,
  ) -> (SupervisorDirective, Vec<Pid>) {
    // Get supervisor strategy dynamically from actor instance
    let strategy = {
      let system = ActorSystemGeneric::from_state(self.system.clone());
      let mut ctx = ActorContextGeneric::new(&system, self.pid);
      self.actor.with_write(|actor| actor.supervisor_strategy(&mut ctx))
    };

    let directive = {
      let mut state = self.state.lock();
      let entry = find_or_insert_stats(&mut state.child_stats, child);
      strategy.handle_failure(entry, error, now)
    };

    let affected = match strategy.kind() {
      | SupervisorStrategyKind::OneForOne => vec![child],
      | SupervisorStrategyKind::AllForOne => self.state.lock().children.clone(),
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
    let mut state = self.state.lock();
    state.child_stats.retain(|(pid, _)| !children.contains(pid));
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

/// Type alias for `ActorCellGeneric` with the default `NoStdToolbox`.
pub type ActorCell = ActorCellGeneric<NoStdToolbox>;
