//! Runtime container responsible for executing an actor instance.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use portable_atomic::{AtomicBool, Ordering};

use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
  actor_prim::{Actor, ActorContext, Pid, actor_ref::ActorRefGeneric},
  dispatcher::{DispatcherGeneric, DispatcherSender},
  error::ActorError,
  event_stream::EventStreamEvent,
  futures::ActorFuture,
  lifecycle::{LifecycleEvent, LifecycleStage},
  mailbox::{MailboxCapacity, MailboxGeneric, MailboxInstrumentation},
  messaging::{
    AnyMessageGeneric, SystemMessage,
    message_invoker::{MessageInvoker, MessageInvokerPipeline},
  },
  props::{ActorFactory, PropsGeneric},
  supervision::{RestartStatistics, SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  system::{ActorSystemGeneric, SystemStateGeneric},
};

type LifecycleAckFuture<TB> = ArcShared<ActorFuture<Result<(), ActorError>, TB>>;

/// Runtime container responsible for executing an actor instance.
pub struct ActorCellGeneric<TB: RuntimeToolbox + 'static> {
  pid:                Pid,
  parent:             Option<Pid>,
  name:               String,
  system:             ArcShared<SystemStateGeneric<TB>>,
  factory:            ArcShared<dyn ActorFactory<TB>>,
  actor:              ToolboxMutex<Box<dyn Actor<TB> + Send + Sync>, TB>,
  pipeline:           MessageInvokerPipeline<TB>,
  mailbox:            ArcShared<MailboxGeneric<TB>>,
  dispatcher:         DispatcherGeneric<TB>,
  sender:             ArcShared<DispatcherSender<TB>>,
  children:           ToolboxMutex<Vec<Pid>, TB>,
  supervisor:         SupervisorStrategy,
  child_stats:        ToolboxMutex<Vec<(Pid, RestartStatistics)>, TB>,
  watchers:           ToolboxMutex<Vec<Pid>, TB>,
  terminated:         AtomicBool,
  pending_create_ack: ToolboxMutex<Option<LifecycleAckFuture<TB>>, TB>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorCellGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorCellGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> ActorCellGeneric<TB> {
  /// Creates a new actor cell using the provided runtime state and props.
  #[must_use]
  pub fn create(
    system: ArcShared<SystemStateGeneric<TB>>,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &PropsGeneric<TB>,
  ) -> ArcShared<Self> {
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
      let instrumentation = MailboxInstrumentation::new(system.clone(), pid, capacity, throughput, warn_threshold);
      mailbox.set_instrumentation(instrumentation);
    }
    let dispatcher = props.dispatcher().build_dispatcher(mailbox.clone());
    let sender = dispatcher.into_sender();
    let factory = props.factory().clone();
    let actor = <TB::MutexFamily as SyncMutexFamily>::create(factory.create());
    let children = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());
    let supervisor = *props.supervisor().strategy();
    let child_stats = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());
    let watchers = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());

    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      system,
      factory,
      actor,
      pipeline: MessageInvokerPipeline::new(),
      mailbox,
      dispatcher,
      sender,
      children,
      supervisor,
      child_stats,
      watchers,
      terminated: AtomicBool::new(false),
      pending_create_ack: <TB::MutexFamily as SyncMutexFamily>::create(None),
    });

    {
      // Dispatcher keeps a shared reference to the invoker for message delivery.
      let invoker: ArcShared<dyn MessageInvoker<TB>> = cell.clone();
      cell.dispatcher.register_invoker(invoker);
    }

    cell
  }

  /// Recreates the actor instance from the stored factory.
  fn recreate_actor(&self) {
    let mut actor = self.actor.lock();
    *actor = self.factory.create();
  }

  /// Returns the pid associated with the cell.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical actor name.
  #[must_use]
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

  /// Produces an actor reference targeting this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRefGeneric<TB> {
    ActorRefGeneric::with_system(self.pid, self.sender.clone(), self.system.clone())
  }

  /// Arms a lifecycle acknowledgement that completes when `SystemMessage::Create` finishes.
  pub(crate) fn prepare_create_ack(&self) -> LifecycleAckFuture<TB> {
    let future = ArcShared::new(ActorFuture::new());
    *self.pending_create_ack.lock() = Some(future.clone());
    future
  }

  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    let mut children = self.children.lock();
    if !children.contains(&pid) {
      children.push(pid);
    }
    let mut stats = self.child_stats.lock();
    find_or_insert_stats(&mut stats, pid);
  }

  /// Removes a child pid from supervision tracking.
  pub fn unregister_child(&self, pid: &Pid) {
    self.children.lock().retain(|child| child != pid);
    self.child_stats.lock().retain(|(child, _)| child != pid);
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.children.lock().clone()
  }

  fn mark_terminated(&self) {
    self.terminated.store(true, Ordering::Release);
  }

  fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  pub(crate) fn handle_watch(&self, watcher: Pid) {
    if self.is_terminated() {
      let _ = self.system.send_system_message(watcher, SystemMessage::Terminated(self.pid));
      return;
    }

    let mut watchers = self.watchers.lock();
    if !watchers.contains(&watcher) {
      watchers.push(watcher);
    }
  }

  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.watchers.lock().retain(|pid| *pid != watcher);
  }

  fn notify_watchers_on_stop(&self) {
    let mut watchers = self.watchers.lock();
    if watchers.is_empty() {
      return;
    }
    let recipients = watchers.clone();
    watchers.clear();
    drop(watchers);

    for watcher in recipients {
      let _ = self.system.send_system_message(watcher, SystemMessage::Terminated(self.pid));
    }
  }

  pub(crate) fn handle_terminated(&self, terminated_pid: Pid) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let result = actor.on_terminated(&mut ctx, terminated_pid);
    drop(actor);
    ctx.clear_reply_to();
    result
  }

  fn handle_create(&self) -> Result<(), ActorError> {
    let outcome = self.run_pre_start(LifecycleStage::Started);
    self.notify_create_result(outcome.clone());
    outcome
  }

  fn handle_recreate(&self) -> Result<(), ActorError> {
    {
      let system = ActorSystemGeneric::from_state(self.system.clone());
      let mut ctx = ActorContext::new(&system, self.pid);
      let mut actor = self.actor.lock();
      actor.post_stop(&mut ctx)?;
      ctx.clear_reply_to();
    }

    self.publish_lifecycle(LifecycleStage::Stopped);
    self.recreate_actor();
    self.run_pre_start(LifecycleStage::Restarted)
  }

  fn notify_create_result(&self, outcome: Result<(), ActorError>) {
    if let Some(future) = self.pending_create_ack.lock().take() {
      future.complete(outcome);
    }
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn watchers_snapshot(&self) -> Vec<Pid> {
    self.watchers.lock().clone()
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let result = actor.post_stop(&mut ctx);
    drop(actor);
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
    self.system.remove_cell(&self.pid);

    if self.system.clear_guardian(self.pid) {
      self.system.mark_terminated();
    }

    result
  }

  fn run_pre_start(&self, stage: LifecycleStage) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let outcome = actor.pre_start(&mut ctx);
    drop(actor);
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

impl<TB: RuntimeToolbox + 'static> MessageInvoker<TB> for ActorCellGeneric<TB> {
  fn invoke_user_message(&self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError> {
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let result = self.pipeline.invoke_user(&mut *actor, &mut ctx, message);
    drop(actor);
    if let Err(ref error) = result {
      system.state().notify_failure(self.pid, error);
    }
    result
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    match message {
      | SystemMessage::Stop => self.handle_stop(),
      | SystemMessage::Create => self.handle_create(),
      | SystemMessage::Recreate => self.handle_recreate(),
      | SystemMessage::Suspend => {
        self.mailbox.suspend();
        Ok(())
      },
      | SystemMessage::Resume => {
        self.mailbox.resume();
        Ok(())
      },
      | SystemMessage::Watch(pid) => {
        self.handle_watch(pid);
        Ok(())
      },
      | SystemMessage::Unwatch(pid) => {
        self.handle_unwatch(pid);
        Ok(())
      },
      | SystemMessage::Terminated(pid) => self.handle_terminated(pid),
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
    let directive = {
      let mut stats = self.child_stats.lock();
      let entry = find_or_insert_stats(&mut stats, child);
      self.supervisor.handle_failure(entry, error, now)
    };

    let affected = match self.supervisor.kind() {
      | SupervisorStrategyKind::OneForOne => vec![child],
      | SupervisorStrategyKind::AllForOne => self.children.lock().clone(),
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
    self.child_stats.lock().retain(|(pid, _)| !children.contains(pid));
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
