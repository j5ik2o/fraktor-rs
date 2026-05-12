//! Runtime container responsible for executing an actor instance.

#[cfg(test)]
#[path = "actor_cell_test.rs"]
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
use core::{any::Any, mem, task::Poll, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, WeakShared};
use portable_atomic::{AtomicBool, Ordering};

use crate::{
  actor::{
    Actor, ActorCellState, ActorCellStateShared, ActorContext, ActorShared, FailedInfo, Pid, ReceiveTimeoutStateShared,
    STASH_OVERFLOW_REASON, STASH_REQUIRES_DEQUE_REASON, SuspendReason, WatchKind, WatchRegistrationKind,
    actor_ref::{ActorRef, ActorRefSenderShared},
    context_pipe::{ContextPipeFuture, ContextPipeTask, ContextPipeTaskId},
    error::{ActorError, ActorErrorReason, PipeSpawnError},
    lifecycle::{LifecycleEvent, LifecycleStage},
    message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
    messaging::{
      ActorIdentity, AnyMessage, Identify, Kill, PoisonPill,
      message_invoker::{MessageInvoker, MessageInvokerPipeline, MessageInvokerShared},
      system_message::{FailureMessageSnapshot, FailurePayload, SystemMessage},
    },
    props::{ActorFactoryShared, Props},
    scheduler::{SchedulerCommand, SchedulerError, SchedulerHandle, SchedulerShared},
    spawn::SpawnError,
    supervision::{RestartStatistics, SupervisorDirective, SupervisorStrategyKind},
  },
  dispatch::{
    dispatcher::{DEFAULT_DISPATCHER_ID, DispatcherSender, MessageDispatcherShared},
    mailbox::{Mailbox, MailboxCapacity, MailboxFactory, MailboxInstrumentation, metrics_event::MailboxPressureEvent},
  },
  event::{logging::LogLevel, stream::EventStreamEvent},
  system::{
    ActorSystem,
    guardian::GuardianKind,
    state::{SystemStateShared, SystemStateWeak, system_state::FailureOutcome},
  },
};

/// Runtime container responsible for executing an actor instance.
///
/// ```compile_fail
/// use fraktor_actor_core_kernel_rs::actor::ActorCell;
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
  mailbox:         ArcShared<Mailbox>,
  dispatcher_id:   String,
  /// Handle to the new-dispatcher tree that owns the cell.
  ///
  /// Every cell is attached to a [`MessageDispatcherShared`] when it is
  /// constructed; `SystemStateShared::remove_cell` calls `detach` on the
  /// inhabitants counter when the cell is dropped.
  new_dispatcher:  MessageDispatcherShared,
  sender:          ActorRefSenderShared,
  receive_timeout: ReceiveTimeoutStateShared,
  state:           ActorCellStateShared,
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
    let system = ActorSystem::from_system_state(self.system());
    ActorContext::new(&system, self.pid).with_receive_timeout_state(self.receive_timeout.as_shared_lock())
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

    let mailbox_factory: ArcShared<dyn MailboxFactory> = if let Some(id) = mailbox_id {
      system.resolve_mailbox(id).map_err(|error| SpawnError::invalid_props(alloc::format!("{error:?}")))?
    } else {
      ArcShared::new(props.mailbox_config().clone())
    };

    let dispatcher_id = Self::resolve_dispatcher_id(&system, parent, props)?;
    // The dispatcher tree owns the ActorRef sender path. A configurator must
    // be registered for the resolved id (`ActorSystemConfig::default()` seeds
    // the in-process inline configurator).
    let new_dispatcher = system.resolve_dispatcher(&dispatcher_id).ok_or_else(|| {
      SpawnError::invalid_props(alloc::format!("no dispatcher configurator registered for id `{dispatcher_id}`"))
    })?;
    // dispatcher 自身が mailbox を用意したい場合 (例: `BalancingDispatcher` は
    // 単一の team queue をラップする sharing mailbox を返す) に先に問い合わせる。
    // per-actor queue を使う dispatcher は `None` を返し、`ActorCell` は
    // `MailboxFactory` ベースの経路にフォールバックする。
    // system 由来の `MailboxSharedSet` を取得し、std adaptor が
    // `ActorSystemConfig::with_mailbox_clock` 経由で install した throughput
    // deadline clock を新規構築の mailbox に伝播させる。bundle の `clock = None`
    // なら deadline enforcement は無効化される (throughput-only fallback)。
    let mailbox_shared_set = system.mailbox_shared_set();
    let mailbox = if let Some(shared_mailbox) = new_dispatcher.try_create_shared_mailbox(&mailbox_shared_set) {
      shared_mailbox
    } else if let Some(id) = mailbox_id {
      let queue =
        system.create_mailbox_queue(id).map_err(|error| SpawnError::invalid_props(alloc::format!("{error:?}")))?;
      ArcShared::new(Mailbox::new_with_queue_and_shared_set(mailbox_factory.policy(), queue, &mailbox_shared_set))
    } else {
      ArcShared::new(
        Mailbox::new_from_factory_with_shared_set(&*mailbox_factory, &mailbox_shared_set)
          .map_err(|error| SpawnError::invalid_props(alloc::format!("{error}")))?,
      )
    };
    {
      let policy = mailbox_factory.policy();
      let capacity = match policy.capacity() {
        | MailboxCapacity::Bounded { capacity } => Some(capacity.get()),
        | MailboxCapacity::Unbounded => None,
      };
      let throughput = policy.throughput_limit().map(|limit| limit.get());
      let warn_threshold = mailbox_factory.warn_threshold().map(|threshold| threshold.get());
      let instrumentation = MailboxInstrumentation::new(system.clone(), pid, capacity, throughput, warn_threshold);
      mailbox.set_instrumentation(instrumentation);
    }
    let actor_ref_sender_shared =
      ActorRefSenderShared::new(Box::new(DispatcherSender::new(new_dispatcher.clone(), mailbox.clone())));
    let Some(actor_factory_shared) = props.factory().cloned() else {
      return Err(SpawnError::invalid_props("actor factory is required"));
    };
    let actor_shared = ActorShared::new(actor_factory_shared.with_write(|f| f.create()));
    let receive_timeout_shared = ReceiveTimeoutStateShared::new(None);
    let actor_cell_state_shared = ActorCellStateShared::new(ActorCellState::new());

    let tags = props.tags().clone();
    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      tags,
      system: system.downgrade(),
      factory: actor_factory_shared,
      actor: actor_shared,
      pipeline: MessageInvokerPipeline::new_with_guard(system.invoke_guard_factory().build()),
      mailbox,
      dispatcher_id,
      new_dispatcher,
      sender: actor_ref_sender_shared,
      receive_timeout: receive_timeout_shared,
      state: actor_cell_state_shared,
      terminated: AtomicBool::new(false),
    });

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
      mailbox_handle.install_actor(cell.downgrade());
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
    system.canonical_dispatcher_id(dispatcher_id).map_err(|error| {
      // alias chain 由来のエラー (AliasChainTooDeep / Unknown) を SpawnError に畳み込む際、
      // 元の DispatchersError の Display を含めて設定ミスの診断を容易にする。
      SpawnError::invalid_props(alloc::format!("dispatcher `{dispatcher_id}`: {error}"))
    })
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

  /// Returns a handle to the mailbox managed by this cell.
  #[must_use]
  pub fn mailbox(&self) -> ArcShared<Mailbox> {
    self.mailbox.clone()
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
      // Pekko parity: `Children.scala:initChild` installs the child into the
      // container and lazily creates the accompanying restart stats.
      state.children_state.add_child(pid);
    });
  }

  /// Removes a child pid from supervision tracking.
  ///
  /// Children still covered by a supervision watch are left in `children_state`
  /// on purpose: the parent's `handle_death_watch_notification` is the sole
  /// consumer of the state change returned by
  /// `remove_child_and_get_state_change`. Consuming it here would drop
  /// `SuspendReason::Recreation` before the `DeathWatchNotification` emitted
  /// by `notify_watchers_on_stop` reaches the parent and the restart flow
  /// would never fire.
  ///
  /// Callers that tear down a child outside the `DeathWatchNotification`
  /// pipeline (e.g. `rollback_spawn` when the spawn handshake failed before
  /// the child ever started) are expected to unwire the supervision watch
  /// via [`ActorCell::unregister_supervision_watching`] *before* invoking
  /// this method. With the supervision watch gone, `watching_contains_pid`
  /// returns `false` and the container entry is removed normally.
  pub fn unregister_child(&self, pid: &Pid) {
    self.state.with_write(|state| {
      if state.watching_contains_pid(*pid) {
        return;
      }
      let _ = state.children_state.remove_child_and_get_state_change(*pid);
    });
  }

  fn stop_child(&self, pid: Pid) {
    // Pekko `ActorCell.stop(actor)` (Children.scala):
    //   if (childrenRefs.getByRef(actor).isDefined) {
    //     if (!childrenRefs.isTerminating) {
    //       childrenRefs = childrenRefs.shallDie(actor)
    //       actor.stop()
    //     }
    //   }
    // Skip when either the pid is not a live child or the container is already
    // terminating (`reason == Termination`), matching Pekko's guard that
    // prevents re-stopping during parent termination.
    let should_stop = self
      .state
      .with_read(|state| state.children_state.children().contains(&pid) && !state.children_state.is_terminating());
    if !should_stop {
      return;
    }
    self.mark_child_dying(pid);
    if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Stop) {
      self.system().record_send_error(Some(pid), &send_error);
    }
  }

  /// Marks `pid` as scheduled to die on the child-registry state machine
  /// (Pekko `childrenRefs.shallDie(actor)`). Exposed for
  /// [`ActorContext::stop_child`] / [`ActorContext::stop_all_children`] so
  /// that explicit child-stop requests upgrade the container to
  /// `Terminating(UserRequest)` before the `Stop` system message is dispatched.
  pub(crate) fn mark_child_dying(&self, pid: Pid) {
    self.state.with_write(|state| state.children_state.shall_die(pid));
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.children_state.children())
  }

  /// Returns whether the child-registry container is in the `Normal` or
  /// `Empty` state (Pekko `ChildrenContainer.isNormal`).
  ///
  /// AC-H2: exposed as an observation API so that tests and supervision
  /// paths can branch on the 4-state machine without reaching into
  /// `state.children_state` directly.
  #[must_use]
  pub fn children_state_is_normal(&self) -> bool {
    self.state.with_read(|state| state.children_state.is_normal())
  }

  /// Returns whether the child-registry container is currently in the
  /// `Terminating` variant (for any [`SuspendReason`]) or in the `Terminated`
  /// variant — i.e. the parent is waiting for its children to die.
  ///
  /// Uses the fraktor-rs convenience predicate
  /// [`ChildrenContainer::is_in_terminating_variant`]; `ChildrenContainer::is_terminating`
  /// retains the narrower Pekko parity semantics (`reason == Termination`).
  #[must_use]
  pub fn children_state_is_terminating(&self) -> bool {
    self.state.with_read(|state| state.children_state.is_in_terminating_variant())
  }

  /// Returns whether the cell currently has a failure recorded (Pekko
  /// `isFailed`).
  ///
  /// AC-H3 extension: both `FailedRef(perpetrator)` and `FailedFatally`
  /// count as failed; only `NoFailedInfo` returns `false`.
  #[must_use]
  pub fn is_failed(&self) -> bool {
    self.state.with_read(|state| matches!(state.failed, FailedInfo::Child(_) | FailedInfo::Fatal))
  }

  /// Returns whether the cell is currently in the `FailedFatally` state
  /// (Pekko `isFailedFatally`).
  ///
  /// AC-H3 extension: a fatal failure prevents any further restart attempt
  /// until `clear_failed` is called (e.g. through `finishCreate` /
  /// `finishRecreate`).
  #[must_use]
  pub fn is_failed_fatally(&self) -> bool {
    self.state.with_read(|state| matches!(state.failed, FailedInfo::Fatal))
  }

  /// Returns the [`Pid`] of the child whose failure is currently being
  /// processed, if any (Pekko `perpetrator`).
  ///
  /// AC-H3 extension: only the `FailedRef(perpetrator)` state yields a pid;
  /// `NoFailedInfo` and `FailedFatally` both return `None`.
  #[must_use]
  pub fn perpetrator(&self) -> Option<Pid> {
    self.state.with_read(|state| match state.failed {
      | FailedInfo::Child(pid) => Some(pid),
      | FailedInfo::None | FailedInfo::Fatal => None,
    })
  }

  /// Records a failure with `perpetrator` unless the cell is already in the
  /// `FailedFatally` state (Pekko `setFailed`).
  ///
  /// AC-H3 extension: fatal failures take priority and are never downgraded
  /// to a `FailedRef` by a subsequent child failure.
  pub fn set_failed(&self, perpetrator: Pid) {
    self.state.with_write(|state| {
      // Pekko parity (`FaultHandling.scala`): `setFailed` guards against
      // overwriting `FailedFatally`, so a later child failure cannot downgrade
      // an already-fatal state.
      if matches!(state.failed, FailedInfo::Fatal) {
        return;
      }
      state.failed = FailedInfo::Child(perpetrator);
    });
  }

  /// Marks the cell as fatally failed (Pekko `setFailedFatally`).
  ///
  /// AC-H3 extension: unconditionally overwrites any prior `FailedRef` state
  /// with `FailedFatally` so that subsequent `set_failed` calls are ignored.
  pub fn set_failed_fatally(&self) {
    self.state.with_write(|state| {
      state.failed = FailedInfo::Fatal;
    });
  }

  /// Clears any recorded failure state (Pekko `clearFailed`).
  ///
  /// AC-H3 extension: unconditionally resets the cell to `NoFailedInfo`,
  /// including the `FailedFatally` state — required by the `finishCreate` /
  /// `finishRecreate` restart completion path.
  pub fn clear_failed(&self) {
    self.state.with_write(|state| {
      state.failed = FailedInfo::None;
    });
  }

  /// Registers `target` as a user-level watch on this cell (Pekko
  /// `DeathWatch.watching += target`).
  ///
  /// AC-H5: user-level entry point; records the `(target, WatchKind::User)`
  /// pair. Idempotent for duplicate calls. Internal supervision watches are
  /// registered separately via [`ActorCellState::register_watching`] with
  /// [`WatchKind::Supervision`].
  pub fn register_watching(&self, target: Pid) {
    self.state.with_write(|state| state.register_watching(target, WatchKind::User));
  }

  /// Removes user-level watching of `target` (Pekko `DeathWatch.unwatch`
  /// parity for the `watching` side only).
  ///
  /// Supervision-level watches registered with [`WatchKind::Supervision`] are
  /// preserved so that `finish_recreate` / `finish_terminate` keep firing.
  ///
  /// Unlike Pekko (which is single-threaded per actor and can safely clear
  /// `terminatedQueued` here), fraktor-rs leaves the dedup marker alone: a
  /// concurrent `handle_death_watch_notification` may have just pushed
  /// `target` into `terminated_queued`, and clearing it from this code path
  /// would allow a duplicate notification to drive `finish_recreate` twice.
  /// The marker is removed naturally when the in-flight notification handler
  /// finishes (see `handle_death_watch_notification`).
  pub fn unregister_watching(&self, target: Pid) {
    self.state.with_write(|state| state.unregister_watching(target, WatchKind::User));
  }

  /// Returns whether this cell has any watch registered for `target`,
  /// regardless of [`WatchKind`].
  #[must_use]
  pub fn is_watching(&self, target: Pid) -> bool {
    self.state.with_read(|state| state.watching_contains_pid(target))
  }

  /// Classifies the current **user-level** watch registration for `target`.
  ///
  /// **User watch only.** Supervision-only entries
  /// (`WatchKind::Supervision`) are treated as
  /// [`WatchRegistrationKind::None`] so that kernel-internal parent/child
  /// bookkeeping cannot spuriously trip the duplicate check in
  /// `ActorContext::watch` / `watch_with`.
  ///
  /// Equivalent to Pekko `DeathWatch.scala:104` `watching.get(actor)` viewed
  /// through the lens of `Option[Any]`:
  ///
  /// | fraktor-rs                           | Pekko                     |
  /// |--------------------------------------|---------------------------|
  /// | [`WatchRegistrationKind::None`]      | `watching.get(ref) == None` (absent) |
  /// | [`WatchRegistrationKind::Plain`]     | `watching(ref) == None`   |
  /// | [`WatchRegistrationKind::WithMessage`] | `watching(ref) == Some(_)` |
  pub(crate) fn watch_registration_kind(&self, target: Pid) -> WatchRegistrationKind {
    self.state.with_read(|state| {
      if !state.watching_contains_user(target) {
        WatchRegistrationKind::None
      } else if state.watch_with_messages.iter().any(|(pid, _)| *pid == target) {
        WatchRegistrationKind::WithMessage
      } else {
        WatchRegistrationKind::Plain
      }
    })
  }

  /// Returns a snapshot of the `terminated_queued` set (Pekko
  /// `terminatedQueued.toSeq`).
  ///
  /// AC-H5: exposed so tests can observe dedup behaviour for
  /// `DeathWatchNotification` delivery.
  #[must_use]
  pub fn terminated_queued(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.terminated_queued.clone())
  }

  /// Registers `parent_pid` as a supervision-kind watcher on this cell.
  ///
  /// Used by `spawn_with_parent` to wire the bidirectional supervision watch
  /// so that when this cell stops, `notify_watchers_on_stop` delivers a
  /// `DeathWatchNotification` to the parent (driving `finish_recreate` /
  /// `finish_terminate`). Idempotent for duplicate calls.
  pub(crate) fn register_supervision_watcher(&self, parent_pid: Pid) {
    self.state.with_write(|state| state.register_watcher(parent_pid, WatchKind::Supervision));
  }

  /// Registers `child_pid` in this cell's `watching` set with
  /// [`WatchKind::Supervision`].
  pub(crate) fn register_supervision_watching(&self, child_pid: Pid) {
    self.state.with_write(|state| state.register_watching(child_pid, WatchKind::Supervision));
  }

  /// Removes the `(child_pid, WatchKind::Supervision)` entry from this cell's
  /// `watching` set. User-level watches (`WatchKind::User`) are preserved.
  pub(crate) fn unregister_supervision_watching(&self, child_pid: Pid) {
    self.state.with_write(|state| state.unregister_watching(child_pid, WatchKind::Supervision));
  }

  /// Recursively propagates `SystemMessage::Suspend` to every registered child.
  ///
  /// Pekko parity: `Children.scala:203-208` `suspendChildren(exceptFor)` — the
  /// parent iterates its children and asks each of them to suspend. Each child
  /// mailbox that processes the resulting `Suspend` then propagates to its own
  /// children through the same `system_invoke` path, which is how grandchildren
  /// get reached (AC-H3-T3).
  ///
  /// Failures from `send_system_message` are logged through
  /// `record_send_error` (same convention as `handle_failure` / `stop_child`).
  /// Per `ignored-return-values.md` we observe every failure: a child whose
  /// mailbox is already closed simply produces a recorded log entry, which is
  /// the Pekko-equivalent "child already dead" outcome.
  pub(crate) fn suspend_children(&self) {
    for pid in self.children() {
      if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Suspend) {
        // Pekko parity: a child that is already stopped is a benign case —
        // `send_system_message` returns `SendError::MailboxClosed` and the
        // parent continues with the remaining children.
        self.system().record_send_error(Some(pid), &send_error);
      }
    }
  }

  /// Recursively propagates `SystemMessage::Resume` to every registered child.
  ///
  /// Pekko parity: `Children.scala:210-216` `resumeChildren(cause, perp)` —
  /// Pekko passes the failing child + cause so a per-child `Resume(cause)` can
  /// target only the perpetrator. fraktor-rs does not yet carry a cause payload
  /// on `SystemMessage::Resume` (AC-H4 responsibility), so every child is
  /// resumed unconditionally, mirroring the simpler case where `perp == null`.
  pub(crate) fn resume_children(&self) {
    for pid in self.children() {
      if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Resume) {
        self.system().record_send_error(Some(pid), &send_error);
      }
    }
  }

  pub(crate) fn snapshot_child_restart_stats(&self, pid: Pid) -> Option<RestartStatistics> {
    self.state.with_read(|state| state.children_state.stats_for(pid).cloned())
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
      if let Err(send_error) =
        self.system().send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))
      {
        self.system().record_send_error(Some(watcher), &send_error);
      }
      return;
    }

    let notify_immediately = self.state.with_write(|state| {
      if self.is_terminated() {
        return true;
      }
      state.register_watcher(watcher, WatchKind::User);
      false
    });
    if notify_immediately
      && let Err(send_error) =
        self.system().send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))
    {
      self.system().record_send_error(Some(watcher), &send_error);
    }
  }

  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.state.with_write(|state| state.unregister_watcher(watcher, WatchKind::User));
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

    for (watcher, _kind) in recipients {
      if let Err(send_error) =
        self.system().send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))
      {
        self.system().record_send_error(Some(watcher), &send_error);
      }
    }
  }

  /// Handles a `SystemMessage::DeathWatchNotification(pid)` for a watched
  /// target (Pekko `DeathWatch.scala:watchedActorTerminated` +
  /// `FaultHandling.scala:handleChildTerminated`).
  ///
  /// Dispatches in the following order:
  ///
  /// 1. Drop the notification silently when `pid` is not in `watching` for any [`WatchKind`].
  /// 2. Drop the notification silently when `pid` is already in `terminated_queued` (dedup).
  /// 3. Atomically remove every `(pid, _)` entry from `watching` and push `pid` into
  ///    `terminated_queued`.
  /// 4. Consume the child-container state transition via
  ///    [`ChildrenContainer::remove_child_and_get_state_change`].
  /// 5. When a [`WatchKind::User`] entry was present, deliver either the custom `watch_with`
  ///    message (via the user mailbox) or call [`Actor::on_terminated`] directly. If only a
  ///    [`WatchKind::Supervision`] entry existed (user revoked their watch via `unwatch` but the
  ///    kernel keeps an internal supervision watch), skip user-facing dispatch and clean up any
  ///    leftover `watch_with` registration.
  /// 6. Remove `pid` from `terminated_queued` so subsequent notifications for a re-registered pid
  ///    can fire again.
  /// 7. When the state transition reported `Some(SuspendReason::Recreation(cause))`, drive
  ///    `finish_recreate` with the cause. `Termination` / `Creation` transitions are left for a
  ///    Phase A3 follow-up.
  pub(crate) fn handle_death_watch_notification(&self, pid: Pid) -> Result<(), ActorError> {
    let Some((has_user_watch, state_change)) = self.state.with_write(|state| {
      if !state.watching_contains_pid(pid) {
        return None;
      }
      if state.terminated_queued.contains(&pid) {
        return None;
      }
      let has_user = state.watching.iter().any(|(existing, kind)| *existing == pid && *kind == WatchKind::User);
      state.watching.retain(|(existing, _)| *existing != pid);
      state.terminated_queued.push(pid);
      Some((has_user, state.children_state.remove_child_and_get_state_change(pid)))
    }) else {
      return Ok(());
    };

    let delivery_result = if has_user_watch {
      let custom_message = self.take_watch_with_message(pid);
      if let Some(message) = custom_message {
        self.actor_ref().try_tell(message).map_err(|error| ActorError::from_send_error(&error))
      } else {
        let mut ctx = self.make_context();
        let result = self.actor.with_write(|actor| actor.on_terminated(&mut ctx, pid));
        ctx.clear_sender();
        result
      }
    } else {
      // Supervision-only observation: user revoked their watch, so no user-facing
      // callback fires. Drop any residual `watch_with` registration for hygiene.
      self.remove_watch_with(pid);
      Ok(())
    };

    self.state.with_write(|state| state.terminated_queued.retain(|existing| *existing != pid));

    if let Some(SuspendReason::Recreation(cause)) = state_change {
      debug_assert!(
        self.state.with_read(|state| state.deferred_recreate_cause.as_ref().is_none_or(|stored| stored == &cause)),
        "deferred_recreate_cause must match the cause returned by remove_child_and_get_state_change",
      );
      // Pekko parity: user-callback delivery (on_terminated / try_tell) and
      // finish_recreate are logically independent. finish_recreate internally
      // reports its own failure to the supervisor via set_failed_fatally +
      // report_failure. If the user callback also failed, surface that error
      // to the caller so the user-visible failure is not silently dropped by
      // the `?` operator on finish_recreate's Err. When the delivery
      // succeeded, propagate the finish_recreate result instead.
      let recreate_result = self.finish_recreate(&cause);
      return match delivery_result {
        | Ok(()) => recreate_result,
        | Err(delivery_error) => Err(delivery_error),
      };
    }
    // TODO(Phase A3): dispatch `Some(SuspendReason::Termination)` →
    // `finish_terminate(pid)` once that path is ported. `SuspendReason::Creation`
    // (Pekko `pre_start` handshake) は該当経路を移植する段階で variant と共に追加する。

    delivery_result
  }

  /// Registers a custom message to deliver when the watched target terminates.
  ///
  /// **Invariant**: `ActorContext::watch_with` performs a
  /// [`watch_registration_kind`](Self::watch_registration_kind) check **before**
  /// invoking this helper, so arriving with an existing entry for `target`
  /// indicates a violation of the duplicate-check contract
  /// (`pekko-death-watch-duplicate-check` Decision 4). In debug builds this
  /// panics; in release builds the existing entry is replaced to preserve
  /// safety but the bug should be fixed upstream.
  pub(crate) fn register_watch_with(&self, target: Pid, message: AnyMessage) {
    self.state.with_write(|state| {
      debug_assert!(
        !state.watch_with_messages.iter().any(|(pid, _)| *pid == target),
        "register_watch_with invariant violated: duplicate entry for {target:?}. \
         ActorContext::watch_with must call watch_registration_kind first.",
      );
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

  /// Pekko `FaultHandling.scala:92-118` faultRecreate: drives the first phase
  /// of the restart state machine.
  ///
  /// The method calls `pre_restart(&mut ctx, &cause)` and either falls through
  /// to `finish_recreate(cause)` immediately (no live children) or defers the
  /// completion by tagging `ChildrenContainer` with
  /// `SuspendReason::Recreation(cause)` and storing the cause in
  /// `deferred_recreate_cause` until the last child terminates.
  pub(crate) fn fault_recreate(&self, cause: &ActorErrorReason) -> Result<(), ActorError> {
    // Pekko parity: when the cell is already marked as fatally failed, Pekko
    // keeps the actor `null` and treats `faultRecreate` as a no-op. fraktor-rs
    // preserves the same semantics so subsequent callbacks do not fire.
    if self.is_failed_fatally() {
      return Ok(());
    }

    {
      let mut ctx = self.make_context();
      ctx.cancel_receive_timeout();
      // Pekko parity under sync dispatch: default `pre_restart` invokes
      // `stop_all_children`, which sends `SystemMessage::Stop` to each child.
      // When the outer invocation is driven via `ActorCellInvoker::system_invoke`
      // (e.g. test direct calls) the executor's `running` flag is not yet held,
      // so the first nested `execute` for the child mailbox would claim the
      // drain-owner slot and drain on the same thread — reentering into the
      // parent before `set_children_termination_reason(Recreation)` runs.
      // `run_with_drive_guard` claims the slot via the existing
      // `ExecutorShared` trampoline for the duration of `pre_restart`, forcing
      // child mailbox work to queue up instead. Production dispatchers already
      // enter the trampoline when `mailbox.run` is scheduled on a worker
      // thread, so this wrap is effectively a no-op there.
      // 範囲制限: この guard が保護するのは親と同一の `ExecutorShared`（=同一 dispatcher）
      // 配下の child のみ。`with_dispatcher_id` で別 dispatcher を割り当てた child の
      // `send_system_message` → その dispatcher 側の `system_dispatch` は親とは別の
      // trampoline を通るため、guard の外で実行され得る。クロス dispatcher 下の
      // 再入防止は各 dispatcher 側の CAS ベース drain-owner 選択が担う。
      let dispatcher = self.new_dispatcher_shared();
      let pre_restart_result =
        dispatcher.run_with_drive_guard(|| self.actor.with_write(|actor| actor.pre_restart(&mut ctx, cause)));
      pre_restart_result?;
      ctx.clear_sender();
    }

    debug_assert!(
      self.mailbox().is_suspended(),
      "fault_recreate expects the mailbox to be suspended (AC-H3 precondition)"
    );

    let deferred = self.state.with_write(|state| {
      state.deferred_recreate_cause = Some(cause.clone());
      state.children_state.set_children_termination_reason(SuspendReason::Recreation(cause.clone()))
    });

    if deferred {
      // `finish_recreate` will fire from `handle_death_watch_notification` once
      // the last live child terminates.
      return Ok(());
    }

    self.finish_recreate(cause)
  }

  /// Pekko `FaultHandling.scala:278-303` finishRecreate: second phase of the
  /// restart state machine. Performs the actual actor recreation and drives
  /// `post_restart`.
  pub(crate) fn finish_recreate(&self, cause: &ActorErrorReason) -> Result<(), ActorError> {
    self.state.with_write(|state| {
      state.deferred_recreate_cause.take();
      // Pekko `FaultHandling.scala:294` parity: at this point
      // `children_state` must no longer be Terminating. Two paths reach
      // finish_recreate:
      //   1. Immediate path from fault_recreate when `set_children_termination_reason` returned false —
      //      the container was Normal/Empty to begin with.
      //   2. Deferred path from handle_death_watch_notification — `remove_child_and_get_state_change`
      //      transitions the container out of Terminating once the last `to_die` child dies.
      // Assert the invariant so a future regression surfaces early.
      debug_assert!(
        !state.children_state.is_in_terminating_variant(),
        "finish_recreate expects children_state to be Normal/Empty/Terminated, not Terminating"
      );
    });

    self.drop_pipe_tasks();
    self.drop_stash_messages();
    self.drop_timer_handles();
    self.drop_watch_with_messages();
    self.publish_lifecycle(LifecycleStage::Stopped);
    self.recreate_actor();
    // Pekko `FaultHandling.scala:173` `finishCreate` / `:284` `finishRecreate`:
    //   try resumeNonRecursive() finally clearFailed()
    // Clears `FailedInfo` (set by `report_failure` via AC-M3's
    // `set_failed(self.pid)` wiring) so the fresh actor instance starts
    // from `FailedInfo::None`. Paired with `SystemMessage::Resume` arm
    // to cover both Restart and Resume supervisor directives.
    self.clear_failed();

    let outcome = {
      let mut ctx = self.make_context();
      let result = self.actor.with_write(|actor| actor.post_restart(&mut ctx, cause));
      ctx.clear_sender();
      result
    };
    match outcome {
      | Ok(()) => {
        // Pekko `FaultHandling.scala:292` と同様に `post_restart` 成功後に mailbox を
        // resume する。先に resume してしまうと、dispatcher 実装によっては再初期化前の
        // actor に user message が配送される可能性がある。
        self.mailbox().resume();
        self.publish_lifecycle(LifecycleStage::Restarted);
        Ok(())
      },
      | Err(error) => {
        // fault_recreate の AC-H3 precondition により mailbox は既に suspended。
        // report_failure は supervisor へ報告する前に mailbox.suspend() を呼ぶため、
        // ここで先に resume して suspend_count を入口時点の値に戻しておかないと、
        // カウンタが二重に増え、supervisor からの単発 Resume で mailbox が再開
        // できず永続的に stuck する。
        self.mailbox().resume();
        self.set_failed_fatally();
        self.report_failure(&error, None);
        Err(error)
      },
    }
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn watchers_snapshot(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.watchers.iter().map(|(pid, _)| *pid).collect())
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

  /// Reports an invocation failure to the supervisor, following Pekko
  /// `FaultHandling.scala:215-234` `handleInvokeFailure` step-by-step:
  ///
  /// 1. `suspendNonRecursive()` (L218) — suspend this actor's mailbox.
  /// 2. `case _ if !isFailed => setFailed(self)` (L222, AC-M3) — record the perpetrator as
  ///    `self.pid` when not already failed. The `is_failed()` guard prevents overwriting a prior
  ///    perpetrator on duplicate reports, and the inner `set_failed` implementation
  ///    (`actor_cell.rs:448`) additionally preserves `FailedInfo::Fatal` against downgrade — the
  ///    two guards compose so that neither existing `Child(_)` nor `Fatal` state is disturbed.
  /// 3. `suspendChildren(...)` (L225, AC-H3) — recursively suspend children.
  /// 4. `sendSystemMessage(Failed(...))` (L231-234) — hand the failure to the supervisor through
  ///    `system.report_failure(payload)`. This always fires (independent of the `isFailed` guard)
  ///    so Pekko's "report on every occurrence" semantics is preserved.
  ///
  /// The AC-H3 extension requires the parent mailbox and every descendant
  /// to be suspended prior to `system.report_failure` so the supervisor
  /// directive sees a fully quiesced subtree.
  fn report_failure(&self, error: &ActorError, snapshot: Option<FailureMessageSnapshot>) {
    // Pekko `FaultHandling.scala:218` suspendNonRecursive()
    self.mailbox().suspend();
    // Pekko `FaultHandling.scala:221-222` handleInvokeFailure:
    //   case _ if !isFailed => setFailed(self); Set.empty
    // fraktor-rs の report_failure は user / system message 処理失敗で
    // 呼ばれる self-failure 経路のため、perpetrator は常に self.pid。
    // child perpetrator 分岐 (Pekko L221) は現行 `FailureMessageSnapshot`
    // に child pid 情報が含まれないため AC-M3 のスコープ外 (Decision 3)。
    // is_failed() guard が既存 perpetrator (Child(_) もしくは Fatal) を
    // overwrite しないことを保証する。
    if !self.is_failed() {
      self.set_failed(self.pid);
    }
    // Pekko `FaultHandling.scala:225` suspendChildren(exceptFor = skip)
    // self-failure 経路のため skip = empty (全子を suspend)。
    self.suspend_children();
    let timestamp = self.system().monotonic_now();
    let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
    // Pekko `FaultHandling.scala:231-234` parent.sendSystemMessage(Failed(...))
    // guard 通過有無に関わらず毎回 supervisor へ通知する (Pekko 同挙動)。
    self.system().report_failure(payload);
  }

  /// Processes a child failure, mirroring Pekko `FaultHandling.scala:305`
  /// `handleFailure(f: Failed)`: runs the supervisor decision, notifies the
  /// actor, and applies the directive.
  fn handle_failure(&self, payload: &FailurePayload) {
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
        // Pekko `SupervisorStrategy.restartChild(..., suspendFirst)`: the
        // originally failing child is already suspended (via its own
        // `report_failure`), but AllForOne siblings must be suspended before
        // `Recreate` arrives so `fault_recreate` observes the AC-H3
        // "suspended mailbox" precondition.
        for target in &affected {
          if *target != payload.child()
            && let Some(sibling_cell) = self.system().cell(target)
          {
            sibling_cell.mailbox().suspend();
            sibling_cell.suspend_children();
          }
        }
        let mut restart_failed = false;
        for target in affected {
          let cause = actor_error.to_reason();
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Recreate(cause)) {
            self.system().record_send_error(Some(target), &send_error);
            restart_failed = true;
          }
        }

        if restart_failed {
          self.system().record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
          let snapshot = payload.message().cloned();
          // Pekko `FaultHandling.scala:62-67` handleInvokeFailure: the
          // supervisor itself is now failing (could not restart the child),
          // so it must suspend its own mailbox + children before reporting
          // upward. `report_failure` centralises that sequence.
          self.report_failure(&actor_error, snapshot);
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
        // Pekko `FaultHandling.scala:62-67` handleInvokeFailure semantics:
        // escalation from this supervisor means it will itself become the
        // subject of a restart decision by its own parent. Suspend the
        // mailbox + children so the grandparent-issued `Recreate` finds the
        // cell in the AC-H3 precondition state for `fault_recreate`.
        self.report_failure(&actor_error, snapshot);
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
  fn invoke(&mut self, message: AnyMessage) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the message
      return Ok(());
    };
    if cell.is_terminated() {
      return Ok(());
    }
    if message.payload().downcast_ref::<PoisonPill>().is_some() {
      return cell.handle_stop();
    }
    if message.payload().downcast_ref::<Kill>().is_some() {
      let snapshot = FailureMessageSnapshot::from_message(&message);
      return cell.handle_kill(Some(snapshot));
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
      // Pekko `dungeon/ReceiveTimeout.scala:40-42` checkReceiveTimeoutIfNeeded:
      //   reschedule = !message.isInstanceOf[NotInfluenceReceiveTimeout] || receiveTimeoutChanged
      // fraktor-rs では marker trait を `AnyMessage::not_influence` 経由で flag に畳み込んでおり、
      // `is_not_influence_receive_timeout() == true` のときは reschedule をスキップする。
      | Ok(()) => {
        if !failure_candidate.is_not_influence_receive_timeout() {
          ctx.reschedule_receive_timeout();
        }
      },
      | Err(error) => {
        let snapshot = FailureMessageSnapshot::from_message(&failure_candidate);
        cell.report_failure(error, Some(snapshot));
      },
    }
    result
  }

  fn system_invoke(&mut self, message: SystemMessage) -> Result<(), ActorError> {
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
        let payload: ArcShared<dyn Any + Send + Sync + 'static> = ArcShared::new(SystemMessage::Kill);
        let snapshot = FailureMessageSnapshot::new(payload, None);
        cell.handle_kill(Some(snapshot))
      },
      | SystemMessage::Stop => cell.handle_stop(),
      | SystemMessage::Create => cell.handle_create(),
      | SystemMessage::Recreate(cause) => cell.fault_recreate(&cause),
      | SystemMessage::Failure(ref payload) => {
        cell.handle_failure(payload);
        Ok(())
      },
      | SystemMessage::Suspend => {
        // Pekko `FaultHandling.scala:124-128` faultSuspend: the mailbox counter
        // has already been updated inside `Mailbox::process_all_system_messages`
        // (MB-H1); here we only perform the AC-H3 recursion into the children.
        cell.suspend_children();
        Ok(())
      },
      | SystemMessage::Resume => {
        // Pekko `FaultHandling.scala:136-153` faultResume: the mailbox counter
        // has already been decremented by the mailbox layer before forwarding
        // (MB-H1).
        //
        // AC-M3 (change pekko-fault-dispatcher-hardening): mirror Pekko's
        // `finally if (causedByFailure ne null) clearFailed()` at
        // `FaultHandling.scala:150`. Because `report_failure` now records
        // `FailedInfo::Child(self.pid)` via `set_failed` (Pekko L222),
        // receiving `Resume` must clear that state so `is_failed()` does not
        // stay stale across supervisor-approved resume directives.
        //
        // Known divergence from Pekko (Decision 5 in design.md):
        //   - Pekko's `clearFailed` (L83-86) preserves `FailedFatally`; fraktor-rs's `clear_failed()` is
        //     unconditional. Accepted because `SystemMessage::Resume` never reaches a cell that remained in
        //     `Fatal` state in production — the only `set_failed_fatally()` production call site is the
        //     `finish_recreate` post_restart-failure path, after which the supervisor typically chooses
        //     Restart/Stop, not Resume.
        //   - Pekko propagates `causedByFailure` through `resumeChildren` so only the originator clears
        //     `_failed`; fraktor-rs's `SystemMessage::Resume` carries no cause, so propagation into
        //     children that independently acquired `FailedInfo::Child(_)` state would over-clear. This race
        //     is narrow (no production readers of `perpetrator()` yet) and accepted for AC-M3 scope. A
        //     future `SystemMessage::Resume { cause: Option<...> }` refactor can restore strict Pekko
        //     parity.
        //
        // Ordering matches Pekko's `try resumeNonRecursive() finally
        // clearFailed(); resumeChildren(...)` — clear before propagation.
        cell.clear_failed();
        cell.resume_children();
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
      | SystemMessage::DeathWatchNotification(pid) => cell.handle_death_watch_notification(pid),
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
        // Pekko parity (`Children.scala:handleChildTerminated`): obtain the
        // restart stats from the container, creating a fresh entry if the
        // child had not been seen yet. `stats_for_mut` returns `None` only on
        // the `Terminated` state, which is unreachable here — a failed child
        // necessarily means the parent is still alive.
        match state.children_state.stats_for_mut(child) {
          | Some(entry) => strategy.handle_failure(entry, error, now),
          | None => {
            // Defensive fallback: if we are somehow in a `Terminated` state,
            // short-circuit to `Stop` to avoid restarting a dead container.
            SupervisorDirective::Stop
          },
        }
      })
    };

    let affected = match strategy.kind() {
      | SupervisorStrategyKind::OneForOne => vec![child],
      | SupervisorStrategyKind::AllForOne => self.state.with_read(|state| state.children_state.children()),
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
    self.state.with_write(|state| {
      // Pekko parity: when the strategy directive is `Stop`, affected children
      // are removed from the container. We drop the returned state-change
      // reasons — AC-H4 will consume them to drive `finishRecreate` /
      // `finishTerminate`.
      for pid in children {
        let _ = state.children_state.remove_child_and_get_state_change(*pid);
      }
    });
  }
}
