//! Remote watcher daemon bridging watch/unwatch requests to remoting control.

use alloc::vec::Vec;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::{
  RemotingConnectionSnapshot, RemotingControl, RemotingControlHandle, RemotingError,
  core::flight_recorder::remoting_flight_recorder::RemotingFlightRecorder,
};

/// Command handled by the remote watcher daemon.
#[derive(Clone)]
pub enum RemoteWatcherMessage {
  /// Registers a watch request for the specified remote path.
  Watch { target: ActorPathParts },
  /// Cancels a previously registered watch.
  Unwatch { target: ActorPathParts },
  /// Requests the latest endpoint snapshot.
  Snapshot,
}

/// Remote watcher implementation bridging watch/unwatch to remoting control.
pub struct RemoteWatcherDaemon<TB: RuntimeToolbox + 'static> {
  control:         RemotingControlHandle<TB>,
  flight_recorder: RemotingFlightRecorder,
  pending_watches: Vec<ActorPathParts>,
}

impl<TB: RuntimeToolbox + 'static> RemoteWatcherDaemon<TB> {
  /// Creates a new daemon bound to the provided control handle.
  #[must_use]
  pub fn new(control: RemotingControlHandle<TB>) -> Self {
    let flight_recorder = control.flight_recorder();
    Self { control, flight_recorder, pending_watches: Vec::new() }
  }

  /// Spawns the daemon under the system guardian.
  pub fn spawn(
    system: &ActorSystemGeneric<TB>,
    control: RemotingControlHandle<TB>,
  ) -> Result<fraktor_actor_rs::core::actor_prim::actor_ref::ActorRefGeneric<TB>, RemotingError> {
    let props = PropsGeneric::from_fn({
      let control = control.clone();
      move || RemoteWatcherDaemon::new(control.clone())
    })
    .with_name("remote-watcher-daemon");

    system
      .spawn_system_actor(&props)
      .map(|child| child.actor_ref().clone())
      .map_err(|_| RemotingError::SystemUnavailable)
  }

  fn handle_watch(&mut self, ctx: &ActorContextGeneric<'_, TB>, target: &ActorPathParts) -> Result<(), RemotingError> {
    self.pending_watches.push(target.clone());
    self.control.associate(target)?;
    self.refresh_snapshot(ctx);
    Ok(())
  }

  fn handle_unwatch(
    &mut self,
    ctx: &ActorContextGeneric<'_, TB>,
    target: &ActorPathParts,
  ) -> Result<(), RemotingError> {
    self.pending_watches.retain(|existing| existing != target);
    self.refresh_snapshot(ctx);
    Ok(())
  }

  fn refresh_snapshot(&self, ctx: &ActorContextGeneric<'_, TB>) {
    let state = ctx.system().state();
    let latest: Vec<RemotingConnectionSnapshot> = state
      .remote_authority_snapshots()
      .into_iter()
      .map(|(authority, status)| RemotingConnectionSnapshot::new(authority, status))
      .collect();
    self.flight_recorder.update_endpoint_snapshot(latest);
  }
}

impl<TB: RuntimeToolbox + 'static> Actor<TB> for RemoteWatcherDaemon<TB> {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(protocol) = message.downcast_ref::<SystemGuardianProtocol<TB>>() {
      if matches!(protocol, SystemGuardianProtocol::TerminationHook) {
        self.pending_watches.clear();
        self.control.publish_shutdown();
      }
      return Ok(());
    }

    if let Some(command) = message.downcast_ref::<RemoteWatcherMessage>() {
      match command {
        | RemoteWatcherMessage::Watch { target } => {
          let _ = self.handle_watch(ctx, target);
        },
        | RemoteWatcherMessage::Unwatch { target } => {
          let _ = self.handle_unwatch(ctx, target);
        },
        | RemoteWatcherMessage::Snapshot => {
          let _ = ctx.reply(AnyMessageGeneric::new(self.flight_recorder.endpoint_snapshot()));
        },
      }
    }

    Ok(())
  }
}
