//! Core actor system orchestration.

use alloc::{boxed::Box, collections::BTreeMap, string::String};

use cellactor_utils_core_rs::sync::ArcShared;
use spin::Mutex;

use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  actor_ref::ActorRef,
  any_owned_message::AnyOwnedMessage,
  dispatcher::Dispatcher,
  mailbox::{Mailbox, MailboxEnqueue, MailboxError},
  message_invoker::MessageInvoker,
  name_registry::NameRegistry,
  pid::Pid,
  props::Props,
  send_error::SendError,
};

/// Shared pointer to the mutable system state guarded by a spin mutex.
pub(crate) type SystemShared = ArcShared<Mutex<ActorSystemState>>;

/// Public actor system entry point.
pub struct ActorSystem {
  state:    SystemShared,
  guardian: Pid,
}

impl ActorSystem {
  /// Creates a new system using the supplied guardian props.
  pub fn new(guardian_props: Props) -> Result<Self, ActorError> {
    let state = ArcShared::new(Mutex::new(ActorSystemState::new()));
    let guardian_pid;
    {
      let mut guard = state.lock();
      guardian_pid = guard.spawn_actor(&state, None, guardian_props)?;
    }
    Ok(Self { state, guardian: guardian_pid })
  }

  /// Returns a reference to the guardian actor.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRef {
    ActorRef::new(self.guardian, self.state.clone())
  }
}

/// Enqueues a user message into the mailbox.
pub(crate) fn enqueue_user(handle: &SystemShared, pid: Pid, message: AnyOwnedMessage) -> Result<(), SendError> {
  let mut entry = {
    let mut guard = handle.lock();
    guard.take_entry(pid)?
  };

  let result = entry.enqueue_user(message);

  let mut guard = handle.lock();
  guard.put_entry(entry);
  result
}

/// Enqueues a system message into the mailbox.
pub(crate) fn enqueue_system(handle: &SystemShared, pid: Pid, message: AnyOwnedMessage) -> Result<(), SendError> {
  let mut entry = {
    let mut guard = handle.lock();
    guard.take_entry(pid)?
  };

  let result = entry.enqueue_system(message);

  let mut guard = handle.lock();
  guard.put_entry(entry);
  result
}

/// Internal state for actor management.
pub(crate) struct ActorSystemState {
  next_pid: u64,
  registry: NameRegistry,
  actors:   BTreeMap<Pid, ActorEntry>,
}

impl ActorSystemState {
  fn new() -> Self {
    Self { next_pid: 1, registry: NameRegistry::new(), actors: BTreeMap::new() }
  }

  fn allocate_pid(&mut self) -> Pid {
    let pid = Pid::new(self.next_pid, 0);
    self.next_pid = self.next_pid.wrapping_add(1);
    pid
  }

  pub(crate) fn spawn_actor(
    &mut self,
    shared: &SystemShared,
    name: Option<String>,
    props: Props,
  ) -> Result<Pid, ActorError> {
    let pid = self.allocate_pid();
    if let Some(ref name_value) = name {
      let _ = self.registry.register(name_value.as_str(), pid).map_err(|_| ActorError::fatal("duplicate_name"))?;
    }
    let mut entry = ActorEntry::new(shared.clone(), pid, name.clone(), props)?;
    entry.ensure_started()?;
    self.actors.insert(pid, entry);
    Ok(pid)
  }

  fn take_entry(&mut self, pid: Pid) -> Result<ActorEntry, SendError> {
    self.actors.remove(&pid).ok_or(SendError::UnknownPid)
  }

  fn put_entry(&mut self, entry: ActorEntry) {
    self.actors.insert(entry.pid, entry);
  }
}

struct ActorEntry {
  system:     SystemShared,
  pid:        Pid,
  _name:      Option<String>,
  actor:      Box<dyn Actor>,
  mailbox:    Mailbox,
  dispatcher: Dispatcher,
  invoker:    MessageInvoker,
  started:    bool,
}

impl ActorEntry {
  fn new(system: SystemShared, pid: Pid, name: Option<String>, props: Props) -> Result<Self, ActorError> {
    let actor = (props.factory())();
    let mailbox = Mailbox::new(*props.mailbox());
    let dispatcher = Dispatcher::new(props.throughput());
    let invoker = MessageInvoker::new();
    Ok(Self { system, pid, _name: name, actor, mailbox, dispatcher, invoker, started: false })
  }

  fn ensure_started(&mut self) -> Result<(), ActorError> {
    if self.started {
      return Ok(());
    }
    let mut ctx = ActorContext::new(&self.pid);
    self.configure_context(&mut ctx);
    self.actor.pre_start(&mut ctx)?;
    self.started = true;
    Ok(())
  }

  fn run_until_idle(&mut self) {
    let mut remaining = self.dispatcher.throughput();
    while remaining > 0 {
      let Some(message) = self.mailbox.dequeue() else {
        break;
      };

      let mut ctx = ActorContext::new(&self.pid);
      self.configure_context(&mut ctx);
      let _ = self.invoker.invoke(self.actor.as_mut(), &mut ctx, &message);
      remaining -= 1;
    }
  }

  fn configure_context(&self, ctx: &mut ActorContext<'_>) {
    ctx.set_system_handle(self.system.clone());
  }

  fn enqueue_user(&mut self, message: AnyOwnedMessage) -> Result<(), SendError> {
    match self.mailbox.enqueue_user(message) {
      | Ok(MailboxEnqueue::DroppedNewest) => return Err(SendError::DroppedNewest),
      | Ok(_) => {},
      | Err(MailboxError::WouldBlock) => return Err(SendError::MailboxFull),
      | Err(MailboxError::Suspended) => return Err(SendError::MailboxSuspended),
    }
    self.ensure_started().map_err(SendError::from)?;
    self.run_until_idle();
    Ok(())
  }

  fn enqueue_system(&mut self, message: AnyOwnedMessage) -> Result<(), SendError> {
    let _ = self.mailbox.enqueue_system(message);
    self.ensure_started().map_err(SendError::from)?;
    self.run_until_idle();
    Ok(())
  }
}
