use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  actor_ref::ActorRef,
  any_message::AnyMessage,
  dispatcher::Dispatcher,
  event_stream_event::EventStreamEvent,
  lifecycle_event::LifecycleEvent,
  lifecycle_stage::LifecycleStage,
  mailbox::Mailbox,
  mailbox_instrumentation::MailboxInstrumentation,
  mailbox_policy::MailboxCapacity,
  message_invoker::{MessageInvoker, MessageInvokerPipeline},
  pid::Pid,
  props::{ActorFactory, Props},
  restart_statistics::RestartStatistics,
  supervisor_strategy::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  system::ActorSystem,
  system_message::SystemMessage,
  system_state::ActorSystemState,
};

/// Runtime container responsible for executing an actor instance.
pub struct ActorCell {
  pid:         Pid,
  parent:      Option<Pid>,
  name:        String,
  system:      ArcShared<ActorSystemState>,
  factory:     ArcShared<dyn ActorFactory>,
  supervisor:  SupervisorStrategy,
  actor:       SpinSyncMutex<Box<dyn Actor + Send + Sync>>,
  pipeline:    MessageInvokerPipeline,
  dispatcher:  Dispatcher,
  sender:      ArcShared<crate::dispatcher::DispatcherSender>,
  children:    SpinSyncMutex<Vec<Pid>>,
  child_stats: SpinSyncMutex<Vec<(Pid, RestartStatistics)>>,
}

impl ActorCell {
  /// Creates a new actor cell using the provided runtime state and props.
  pub fn create(
    system: ArcShared<ActorSystemState>,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &Props,
  ) -> ArcShared<Self> {
    let mailbox = ArcShared::new(Mailbox::new(props.mailbox().policy()));
    Self::configure_mailbox(&mailbox, &system, pid, props);
    let dispatcher = props.dispatcher().build_dispatcher(mailbox);
    let sender = dispatcher.into_sender();
    let factory = props.factory().clone();
    let supervisor = *props.supervisor().strategy();
    let actor = factory.create();

    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      system,
      factory,
      supervisor,
      actor: SpinSyncMutex::new(actor),
      pipeline: MessageInvokerPipeline::new(),
      dispatcher,
      sender,
      children: SpinSyncMutex::new(Vec::new()),
      child_stats: SpinSyncMutex::new(Vec::new()),
    });

    {
      let invoker: ArcShared<dyn MessageInvoker> = cell.clone();
      cell.dispatcher.register_invoker(invoker);
    }

    cell
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

  /// Produces an [`ActorRef`] pointing at this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRef {
    ActorRef::with_system(self.pid, self.sender.clone(), self.system.clone())
  }

  /// Returns the dispatcher associated with this cell.
  #[must_use]
  pub fn dispatcher(&self) -> Dispatcher {
    self.dispatcher.clone()
  }

  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    {
      let mut children = self.children.lock();
      if !children.contains(&pid) {
        children.push(pid);
      }
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

  /// Handles a child failure and determines the supervision directive.
  #[must_use]
  pub fn handle_child_failure(&self, child: Pid, error: &ActorError, now: Duration) -> (SupervisorDirective, Vec<Pid>) {
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

  /// Restarts the actor by invoking lifecycle hooks.
  ///
  /// # Errors
  ///
  /// Returns an error if `post_stop` or `pre_start` fails.
  pub fn restart(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());

    {
      let mut ctx = ActorContext::new(&system, self.pid);
      let mut actor = self.actor.lock();
      actor.post_stop(&mut ctx)?;
    }

    self.publish_lifecycle(LifecycleStage::Stopped);

    {
      let mut actor_slot = self.actor.lock();
      *actor_slot = self.factory.create();
    }

    self.run_pre_start(LifecycleStage::Restarted)
  }

  /// Executes the actor's `pre_start` hook.
  ///
  /// # Errors
  ///
  /// Returns an error if the hook fails.
  pub fn pre_start(&self) -> Result<(), ActorError> {
    self.run_pre_start(LifecycleStage::Started)
  }

  fn run_pre_start(&self, stage: LifecycleStage) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let outcome = actor.pre_start(&mut ctx);
    if outcome.is_ok() {
      self.publish_lifecycle(stage);
    }
    outcome
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let outcome = actor.post_stop(&mut ctx);
    drop(actor);
    if outcome.is_ok() {
      self.publish_lifecycle(LifecycleStage::Stopped);
    }
    for child in self.children() {
      let _ = self.system.send_system_message(child, SystemMessage::Stop);
    }
    if let Some(parent) = self.parent {
      self.system.unregister_child(Some(parent), self.pid);
    }
    self.system.release_name(self.parent, &self.name);
    self.system.remove_cell(&self.pid);
    if self.system.clear_guardian(self.pid) {
      self.system.mark_terminated();
    }
    outcome
  }

  fn clear_child_stats(&self, children: &[Pid]) {
    if children.is_empty() {
      return;
    }
    self.child_stats.lock().retain(|(pid, _)| !children.iter().any(|target| target == pid));
  }

  fn configure_mailbox(mailbox: &ArcShared<Mailbox>, system: &ArcShared<ActorSystemState>, pid: Pid, props: &Props) {
    let policy = props.mailbox().policy();
    let capacity = match policy.capacity() {
      | MailboxCapacity::Bounded { capacity } => Some(capacity.get()),
      | MailboxCapacity::Unbounded => None,
    };
    let throughput = policy.throughput_limit().map(|limit| limit.get());
    let warn_threshold = props.mailbox().warn_threshold().map(|threshold| threshold.get());
    let instrumentation =
      MailboxInstrumentation::new(system.event_stream(), system.clone(), pid, capacity, throughput, warn_threshold);
    mailbox.set_instrumentation(instrumentation);
  }

  fn publish_lifecycle(&self, stage: LifecycleStage) {
    let timestamp = self.system.monotonic_now();
    let event = LifecycleEvent::new(self.pid, self.parent, self.name.clone(), stage, timestamp);
    self.system.publish_event(&EventStreamEvent::Lifecycle(event));
  }
}

impl MessageInvoker for ActorCell {
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let result = self.pipeline.invoke_user(&mut *actor, &mut ctx, message);
    if let Err(ref error) = result {
      drop(actor);
      self.system.notify_failure(self.pid, error);
    } else {
      drop(actor);
    }
    result
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    match message {
      | SystemMessage::Stop => self.handle_stop(),
      | SystemMessage::Suspend => {
        self.dispatcher.mailbox().suspend();
        Ok(())
      },
      | SystemMessage::Resume => {
        self.dispatcher.mailbox().resume();
        Ok(())
      },
    }
  }
}

#[allow(clippy::needless_range_loop)]
fn find_or_insert_stats(entries: &mut Vec<(Pid, RestartStatistics)>, pid: Pid) -> &mut RestartStatistics {
  let len = entries.len();
  for index in 0..len {
    if entries[index].0 == pid {
      return &mut entries[index].1;
    }
  }
  entries.push((pid, RestartStatistics::new()));
  let new_len = entries.len();
  &mut entries[new_len - 1].1
}
