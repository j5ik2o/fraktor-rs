//! Runtime container responsible for executing an actor instance.

use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeSet, string::String};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};
use portable_atomic::AtomicBool;

use crate::{
  actor::{
    ActorCellState, ActorCellStateShared, ActorContext, ActorShared, Pid, ReceiveTimeoutStateShared,
    actor_cell_dispatch::install_invoker,
    actor_ref::{ActorRef, ActorRefSenderShared},
    messaging::message_invoker::MessageInvokerPipeline,
    props::{ActorFactoryShared, Props},
    scheduler::SchedulerShared,
    spawn::SpawnError,
  },
  dispatch::{
    dispatcher::{DEFAULT_DISPATCHER_ID, DispatcherSender, MessageDispatcherShared},
    mailbox::{Mailbox, MailboxCapacity, MailboxFactory, MailboxInstrumentation},
  },
  system::{
    ActorSystem,
    state::{SystemStateShared, SystemStateWeak},
  },
};

#[cfg(test)]
#[path = "actor_cell_test.rs"]
pub(crate) mod tests;

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
  pub(super) pid:             Pid,
  pub(super) parent:          Option<Pid>,
  pub(super) name:            String,
  pub(super) tags:            BTreeSet<String>,
  pub(super) system:          SystemStateWeak,
  pub(super) factory:         ActorFactoryShared,
  pub(super) actor:           ActorShared,
  pub(super) pipeline:        MessageInvokerPipeline,
  pub(super) mailbox:         ArcShared<Mailbox>,
  pub(super) dispatcher_id:   String,
  /// Handle to the new-dispatcher tree that owns the cell.
  ///
  /// Every cell is attached to a [`MessageDispatcherShared`] when it is
  /// constructed; `SystemStateShared::remove_cell` calls `detach` on the
  /// inhabitants counter when the cell is dropped.
  pub(super) new_dispatcher:  MessageDispatcherShared,
  pub(super) sender:          ActorRefSenderShared,
  pub(super) receive_timeout: ReceiveTimeoutStateShared,
  pub(super) state:           ActorCellStateShared,
  pub(super) terminated:      AtomicBool,
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

  pub(super) fn make_context(&self) -> ActorContext<'_> {
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
      install_invoker(&cell, &mailbox_handle);
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
  pub(super) fn recreate_actor(&self) {
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
}
