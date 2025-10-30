use alloc::{boxed::Box, string::String};

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  actor_ref::ActorRef,
  any_message::AnyOwnedMessage,
  dispatcher::{Dispatcher, InlineExecutor},
  mailbox::Mailbox,
  message_invoker::{MessageInvoker, MessageInvokerPipeline},
  pid::Pid,
  props::Props,
  system::ActorSystem,
  system_message::SystemMessage,
  system_state::ActorSystemState,
};

/// Runtime container responsible for executing an actor instance.
pub struct ActorCell {
  pid:        Pid,
  parent:     Option<Pid>,
  name:       String,
  system:     ArcShared<ActorSystemState>,
  actor:      SpinSyncMutex<Box<dyn Actor + Send + Sync>>,
  pipeline:   MessageInvokerPipeline,
  dispatcher: Dispatcher,
  sender:     ArcShared<crate::dispatcher::DispatcherSender>,
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
    let executor: ArcShared<dyn crate::dispatcher::DispatchExecutor> = ArcShared::new(InlineExecutor::new());
    let dispatcher = Dispatcher::new(mailbox, executor);
    let sender = dispatcher.into_sender();
    let actor = props.factory().create();

    let cell = ArcShared::new(Self {
      pid,
      parent,
      name,
      system,
      actor: SpinSyncMutex::new(actor),
      pipeline: MessageInvokerPipeline::new(),
      dispatcher,
      sender,
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

  /// Produces an [`ActorRef`] pointing at this cell.
  #[must_use]
  pub fn actor_ref(&self) -> ActorRef {
    ActorRef::new(self.pid, self.sender.clone())
  }

  /// Executes the actor's `pre_start` hook.
  ///
  /// # Errors
  ///
  /// Returns an error if the hook fails.
  pub fn pre_start(&self) -> Result<(), ActorError> {
    self.run_pre_start()
  }

  fn run_pre_start(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    actor.pre_start(&mut ctx)
  }

  fn handle_stop(&self) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    let outcome = actor.post_stop(&mut ctx);
    drop(actor);
    self.system.release_name(self.parent, &self.name);
    self.system.remove_cell(&self.pid);
    outcome
  }
}

impl MessageInvoker for ActorCell {
  fn invoke_user_message(&self, message: AnyOwnedMessage) -> Result<(), ActorError> {
    let system = ActorSystem::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);
    let mut actor = self.actor.lock();
    self.pipeline.invoke_user(&mut *actor, &mut ctx, message)
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    match message {
      | SystemMessage::Stop => self.handle_stop(),
      | SystemMessage::Suspend | SystemMessage::Resume => Ok(()),
    }
  }
}
