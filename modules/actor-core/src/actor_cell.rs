//! Runtime container responsible for executing an actor instance.

use alloc::{boxed::Box, string::String, vec::Vec};

use cellactor_utils_core_rs::sync::{sync_mutex_like::SyncMutexLike, ArcShared, SyncMutexFamily};

use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  actor_ref::ActorRef,
  any_message::AnyMessage,
  dispatcher::{Dispatcher, DispatcherSender},
  mailbox::Mailbox,
  message_invoker::MessageInvoker,
  pid::Pid,
  props::{ActorFactory, Props},
  system::ActorSystem,
  system_message::SystemMessage,
  system_state::SystemState,
  RuntimeToolbox, ToolboxMutex,
};

/// Runtime container responsible for executing an actor instance.
pub struct ActorCell<TB: RuntimeToolbox + 'static> {
  pid:        Pid,
  parent:     Option<Pid>,
  name:       String,
  system:     ArcShared<SystemState<TB>>,
  factory:    ArcShared<dyn ActorFactory<TB>>,
  actor:      ToolboxMutex<Box<dyn Actor<TB> + Send + Sync>, TB>,
  mailbox:    ArcShared<Mailbox<TB>>,
  dispatcher: Dispatcher<TB>,
  sender:     ArcShared<DispatcherSender<TB>>,
  children:   ToolboxMutex<Vec<Pid>, TB>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorCell<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorCell<TB> {}

impl<TB: RuntimeToolbox + 'static> ActorCell<TB> {
  /// Creates a new actor cell using the provided runtime state and props.
  #[must_use]
  pub fn create(
    system: ArcShared<SystemState<TB>>,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &Props<TB>,
  ) -> ArcShared<Self> {
    let mailbox = ArcShared::new(Mailbox::new(props.mailbox_policy()));
    let dispatcher = props.dispatcher().build_dispatcher(mailbox.clone());
    let sender = dispatcher.into_sender();
    let factory = props.factory().clone();
    let actor = <TB::MutexFamily as SyncMutexFamily>::create(factory.create());
    let children = <TB::MutexFamily as SyncMutexFamily>::create(Vec::new());

    let cell =
      ArcShared::new(Self { pid, parent, name, system, factory, actor, mailbox, dispatcher, sender, children });

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
  pub fn mailbox(&self) -> ArcShared<Mailbox<TB>> {
    self.mailbox.clone()
  }

  /// Returns the dispatcher associated with this cell.
  #[must_use]
  pub fn dispatcher(&self) -> Dispatcher<TB> {
    self.dispatcher.clone()
  }

  /// Produces an actor reference targeting this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRef<TB> {
    ActorRef::with_system(self.pid, self.sender.clone(), self.system.clone())
  }

  /// Runs the actor's `pre_start` hook.
  pub fn pre_start(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let outcome = actor.pre_start(&mut ctx);
    drop(actor);
    ctx.clear_reply_to();
    outcome
  }

  /// Restarts the actor with a freshly created instance.
  pub fn restart(&self) -> Result<(), ActorError> {
    self.recreate_actor();
    self.pre_start()
  }

  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    let mut children = self.children.lock();
    if !children.contains(&pid) {
      children.push(pid);
    }
  }

  /// Removes a child pid from supervision tracking.
  pub fn unregister_child(&self, pid: &Pid) {
    self.children.lock().retain(|child| child != pid);
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.children.lock().clone()
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let result = actor.post_stop(&mut ctx);
    drop(actor);
    ctx.clear_reply_to();

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

    result
  }
}

impl<TB: RuntimeToolbox + 'static> MessageInvoker<TB> for ActorCell<TB> {
  fn invoke_user_message(&self, message: AnyMessage<TB>) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    ctx.set_reply_to(message.reply_to().cloned());

    let mut actor = self.actor.lock();
    let result = actor.receive(&mut ctx, message.as_view());
    drop(actor);
    ctx.clear_reply_to();
    result
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    match message {
      | SystemMessage::Stop => self.handle_stop(),
      | SystemMessage::Suspend => {
        self.mailbox.suspend();
        Ok(())
      },
      | SystemMessage::Resume => {
        self.mailbox.resume();
        Ok(())
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use alloc::string::ToString;

  use cellactor_utils_core_rs::sync::ArcShared;

  use super::ActorCell;
  use crate::{
    actor::Actor, actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessageView, pid::Pid,
  };

  struct ProbeActor;

  impl Actor for ProbeActor {
    fn receive(
      &mut self,
      _ctx: &mut ActorContext<'_, crate::NoStdToolbox>,
      _message: AnyMessageView<'_>,
    ) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[test]
  fn actor_cell_holds_components() {
    let system = ArcShared::new(crate::SystemState::<crate::NoStdToolbox>::new());
    let props = crate::props::Props::<crate::NoStdToolbox>::from_fn(|| ProbeActor);
    let cell = ActorCell::create(system, Pid::new(1, 0), None, "worker".to_string(), &props);

    assert_eq!(cell.pid(), Pid::new(1, 0));
    assert_eq!(cell.name(), "worker");
    assert!(cell.parent().is_none());
    assert_eq!(cell.mailbox().system_len(), 0);
    assert_eq!(cell.dispatcher().mailbox().system_len(), 0);
  }
}
