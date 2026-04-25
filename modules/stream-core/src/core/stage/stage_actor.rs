#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::mem;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, Kill, PoisonPill, system_message::SystemMessage},
  },
  system::{ActorSystem, state::SystemStateShared},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{StageActorEnvelope, StageActorReceive};
use crate::core::StreamError;

/// Actor-like handle bound to a graph stage lifecycle.
#[derive(Clone)]
pub struct StageActor {
  actor_ref: ActorRef,
  system:    SystemStateShared,
  temp_name: String,
  state:     StageActorStateShared,
  receive:   StageActorReceiveShared,
}

impl StageActor {
  /// Creates a stage actor registered under the materializer's `/temp` registry.
  #[must_use]
  pub(in crate::core) fn new(system: &ActorSystem, receive: Box<dyn StageActorReceive>) -> Self {
    let system_state = system.state();
    let pid = system_state.allocate_pid();
    let state = StageActorStateShared::new();
    let receive = StageActorReceiveShared::new(receive);
    let sender = StageActorSender::new(state.clone());
    let actor_ref = ActorRef::from_shared(pid, ActorRefSenderShared::new(Box::new(sender)), &system_state);
    let temp_name = system_state.register_temp_actor(actor_ref.clone());
    Self { actor_ref, system: system_state, temp_name, state, receive }
  }

  /// Returns the actor reference used to send messages to this stage actor.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor_ref
  }

  /// Replaces the receive callback.
  pub fn r#become(&self, receive: Box<dyn StageActorReceive>) {
    self.receive.replace(receive);
  }

  /// Watches another actor from this stage actor.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the watch system message cannot be delivered.
  pub fn watch(&self, actor: &ActorRef) -> Result<(), StreamError> {
    let target = actor.pid();
    self.state.add_watching(target);
    match self.system.send_system_message(target, SystemMessage::Watch(self.actor_ref.pid())) {
      | Ok(()) => Ok(()),
      | Err(error) => {
        self.state.remove_watching(target);
        Err(StreamError::from_send_error(&error))
      },
    }
  }

  /// Stops watching another actor from this stage actor.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the unwatch system message cannot be delivered.
  pub fn unwatch(&self, actor: &ActorRef) -> Result<(), StreamError> {
    let target = actor.pid();
    let result = self.system.send_system_message(target, SystemMessage::Unwatch(self.actor_ref.pid()));
    result.map_err(|error| StreamError::from_send_error(&error))?;
    self.state.remove_watching(target);
    Ok(())
  }

  /// Stops this stage actor and notifies its watchers.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when a watcher notification cannot be delivered.
  pub fn stop(&self) -> Result<(), StreamError> {
    let Some(watchers) = self.state.stop_and_take_watchers() else {
      return Ok(());
    };
    self.system.unregister_temp_actor(&self.temp_name);

    let mut first_error = None;
    for watcher in watchers {
      if let Err(error) =
        self.system.send_system_message(watcher, SystemMessage::DeathWatchNotification(self.actor_ref.pid()))
        && first_error.is_none()
      {
        first_error = Some(StreamError::from_send_error(&error));
      }
    }

    match first_error {
      | Some(error) => Err(error),
      | None => Ok(()),
    }
  }

  pub(in crate::core) fn drain_pending(&self) -> Result<(), StreamError> {
    let envelopes = self.state.take_inbox();
    for envelope in envelopes {
      self.receive.receive(envelope)?;
    }
    Ok(())
  }
}

struct StageActorSender {
  state: StageActorStateShared,
}

impl StageActorSender {
  const fn new(state: StageActorStateShared) -> Self {
    Self { state }
  }

  fn ignored_control(message: &AnyMessage) -> bool {
    message.downcast_ref::<PoisonPill>().is_some()
      || message.downcast_ref::<Kill>().is_some()
      || matches!(message.downcast_ref::<SystemMessage>(), Some(SystemMessage::PoisonPill | SystemMessage::Kill))
  }
}

impl ActorRefSender for StageActorSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if Self::ignored_control(&message) {
      return Ok(SendOutcome::Delivered);
    }

    if let Some(system_message) = message.downcast_ref::<SystemMessage>().cloned() {
      match system_message {
        | SystemMessage::Watch(watcher) => {
          self.state.add_watcher(watcher);
          return Ok(SendOutcome::Delivered);
        },
        | SystemMessage::Unwatch(watcher) => {
          self.state.remove_watcher(watcher);
          return Ok(SendOutcome::Delivered);
        },
        | SystemMessage::DeathWatchNotification(target) => {
          if !self.state.remove_watching(target) {
            return Ok(SendOutcome::Delivered);
          }
        },
        | _ => {},
      }
    }

    let sender = message.sender().cloned().unwrap_or_else(ActorRef::no_sender);
    self.state.enqueue(StageActorEnvelope::new(sender, message)).map_err(SendError::closed)?;
    Ok(SendOutcome::Delivered)
  }
}

#[derive(Clone)]
struct StageActorStateShared {
  inner: ArcShared<SpinSyncMutex<StageActorState>>,
}

impl StageActorStateShared {
  fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(StageActorState::new())) }
  }

  fn enqueue(&self, envelope: StageActorEnvelope) -> Result<(), AnyMessage> {
    self.inner.lock().enqueue(envelope)
  }

  fn take_inbox(&self) -> Vec<StageActorEnvelope> {
    self.inner.lock().take_inbox()
  }

  fn add_watcher(&self, watcher: Pid) {
    self.inner.lock().add_watcher(watcher);
  }

  fn remove_watcher(&self, watcher: Pid) {
    self.inner.lock().remove_watcher(watcher);
  }

  fn add_watching(&self, target: Pid) {
    self.inner.lock().add_watching(target);
  }

  fn remove_watching(&self, target: Pid) -> bool {
    self.inner.lock().remove_watching(target)
  }

  fn stop_and_take_watchers(&self) -> Option<Vec<Pid>> {
    self.inner.lock().stop_and_take_watchers()
  }
}

struct StageActorState {
  inbox:    Vec<StageActorEnvelope>,
  watchers: Vec<Pid>,
  watching: Vec<Pid>,
  stopped:  bool,
}

impl StageActorState {
  const fn new() -> Self {
    Self { inbox: Vec::new(), watchers: Vec::new(), watching: Vec::new(), stopped: false }
  }

  fn enqueue(&mut self, envelope: StageActorEnvelope) -> Result<(), AnyMessage> {
    if self.stopped {
      return Err(envelope.into_message());
    }
    self.inbox.push(envelope);
    Ok(())
  }

  fn take_inbox(&mut self) -> Vec<StageActorEnvelope> {
    mem::take(&mut self.inbox)
  }

  fn add_watcher(&mut self, watcher: Pid) {
    if !self.watchers.contains(&watcher) {
      self.watchers.push(watcher);
    }
  }

  fn remove_watcher(&mut self, watcher: Pid) {
    self.watchers.retain(|pid| *pid != watcher);
  }

  fn add_watching(&mut self, target: Pid) {
    if !self.watching.contains(&target) {
      self.watching.push(target);
    }
  }

  fn remove_watching(&mut self, target: Pid) -> bool {
    let before = self.watching.len();
    self.watching.retain(|pid| *pid != target);
    self.watching.len() != before
  }

  fn stop_and_take_watchers(&mut self) -> Option<Vec<Pid>> {
    if self.stopped {
      return None;
    }
    self.stopped = true;
    self.inbox.clear();
    Some(mem::take(&mut self.watchers))
  }
}

#[derive(Clone)]
struct StageActorReceiveShared {
  inner: ArcShared<SpinSyncMutex<Box<dyn StageActorReceive>>>,
}

impl StageActorReceiveShared {
  fn new(receive: Box<dyn StageActorReceive>) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(receive)) }
  }

  fn replace(&self, receive: Box<dyn StageActorReceive>) {
    *self.inner.lock() = receive;
  }

  fn receive(&self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    self.inner.lock().receive(envelope)
  }
}
