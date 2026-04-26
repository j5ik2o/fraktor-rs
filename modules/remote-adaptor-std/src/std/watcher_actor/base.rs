//! Tokio actor that owns the pure `WatcherState`.

use fraktor_remote_core_rs::core::{
  address::Address,
  failure_detector::PhiAccrualFailureDetector,
  watcher::{WatcherCommand, WatcherEffect, WatcherState},
};
use tokio::{
  sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
  task::JoinHandle,
};

use crate::std::watcher_actor::watcher_actor_handle::WatcherActorHandle;

/// Tokio-based actor that drives the pure [`WatcherState`].
///
/// `WatcherActor::spawn` consumes the actor and starts a single tokio task
/// that owns the state. All access goes through the returned
/// [`WatcherActorHandle`], which serialises commands by pushing them onto an
/// `mpsc::UnboundedSender`. Because every transition is processed inside the
/// task, the inner `WatcherState`'s `&mut self` contract holds without any
/// extra locking.
pub struct WatcherActor {
  state:      WatcherState,
  command_tx: UnboundedSender<WatcherCommand>,
  command_rx: UnboundedReceiver<WatcherCommand>,
  effect_tx:  UnboundedSender<WatcherEffect>,
}

impl WatcherActor {
  /// Creates a new actor backed by `state`. Effects produced by the state
  /// machine are forwarded to `effect_tx`.
  #[must_use]
  pub fn new(state: WatcherState, effect_tx: UnboundedSender<WatcherEffect>) -> Self {
    let (command_tx, command_rx) = mpsc::unbounded_channel::<WatcherCommand>();
    Self { state, command_tx, command_rx, effect_tx }
  }

  /// Creates a new actor whose detector factory uses sensible Pekko-style
  /// defaults. Convenience constructor for tests and simple wiring.
  #[must_use]
  pub fn with_default_detectors(effect_tx: UnboundedSender<WatcherEffect>) -> Self {
    let state = WatcherState::new(default_detector_factory);
    Self::new(state, effect_tx)
  }

  /// Returns a clonable handle for submitting commands to the actor.
  #[must_use]
  pub fn handle(&self) -> WatcherActorHandle {
    WatcherActorHandle::new(self.command_tx.clone())
  }

  /// Consumes the actor and spawns the running task. Returns the handle
  /// alongside the `JoinHandle` of the spawned task so the caller can
  /// gracefully shut the actor down.
  pub fn spawn(mut self) -> (WatcherActorHandle, JoinHandle<()>) {
    let handle = self.handle();
    let task = tokio::spawn(async move {
      while let Some(command) = self.command_rx.recv().await {
        let effects = self.state.handle(command);
        for effect in effects {
          if self.effect_tx.send(effect).is_err() {
            // Effect consumer dropped — shut down the actor gracefully.
            return;
          }
        }
      }
    });
    (handle, task)
  }
}

fn default_detector_factory(address: &Address) -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::with_monitored_address(address.to_string(), 8.0, 200, 100, 0, 1000)
}
