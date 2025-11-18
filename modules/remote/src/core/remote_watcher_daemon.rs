//! Watches remote actors on behalf of local watchers.

use alloc::vec::Vec;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, Pid, actor_path::ActorPathParts, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  remote_watcher_command::RemoteWatcherCommand, remoting_control::RemotingControl,
  remoting_control_handle::RemotingControlHandle, remoting_error::RemotingError,
};

/// System actor that proxies watch/unwatch commands to the remoting control plane.
pub(crate) struct RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:  RemotingControlHandle<TB>,
  #[allow(dead_code)]
  watchers: Vec<(Pid, ActorPathParts)>,
}

impl<TB> RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(control: RemotingControlHandle<TB>) -> Self {
    Self { control, watchers: Vec::new() }
  }

  /// Spawns the daemon under the system guardian hierarchy.
  pub(crate) fn spawn(
    system: &ActorSystemGeneric<TB>,
    control: RemotingControlHandle<TB>,
  ) -> Result<ActorRefGeneric<TB>, RemotingError> {
    let props = PropsGeneric::from_fn({
      let handle = control.clone();
      move || RemoteWatcherDaemon::new(handle.clone())
    })
    .with_name("remote-watcher-daemon");
    let actor = system.extended().spawn_system_actor(&props).map_err(RemotingError::from)?;
    Ok(actor.actor_ref().clone())
  }
}

impl<TB> Actor<TB> for RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<RemoteWatcherCommand>() {
      match command {
        | RemoteWatcherCommand::Watch { target, .. } => {
          let _ = self.control.associate(target);
        },
        | RemoteWatcherCommand::Unwatch { target: _, .. } => {},
      }
    }
    Ok(())
  }
}
