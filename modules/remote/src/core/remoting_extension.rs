//! Actor system extension provisioning remoting handles.

use fraktor_actor_rs::core::{extension::Extension, system::ActorSystemGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::{
  RemotingControl, RemotingControlHandle, RemotingExtensionConfig, core::endpoint_supervisor::EndpointSupervisor,
};

/// Extension registered inside an actor system to expose remoting controls.
pub struct RemotingExtension<TB: RuntimeToolbox + 'static> {
  control: RemotingControlHandle<TB>,
  _config: RemotingExtensionConfig,
}

impl<TB: RuntimeToolbox + 'static> RemotingExtension<TB> {
  #[must_use]
  pub(crate) fn new(system: &ActorSystemGeneric<TB>, config: RemotingExtensionConfig) -> Self {
    let control = RemotingControlHandle::new(system, config.clone());
    if let Ok(supervisor) = EndpointSupervisor::spawn(system, control.clone()) {
      control.set_supervisor(supervisor);
    }
    for listener in config.backpressure_listeners() {
      control.register_backpressure_listener(listener.clone());
    }
    if config.auto_start() {
      let _ = control.start();
    }
    Self { control, _config: config }
  }

  /// Returns a clonable handle exposing [`RemotingControl`] operations.
  #[must_use]
  pub fn handle(&self) -> RemotingControlHandle<TB> {
    self.control.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for RemotingExtension<TB> {}

#[cfg(test)]
mod tests;
