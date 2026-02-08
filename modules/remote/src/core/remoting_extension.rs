//! Extension entry point wiring remoting control and supervisor actors.

mod config;
mod control;
mod control_handle;
mod error;
#[cfg(test)]
mod tests;

use alloc::string::String;

pub use config::RemotingExtensionConfig;
pub use control::{RemotingControl, RemotingControlShared};
pub use control_handle::RemotingControlHandle;
pub use error::RemotingError;
use fraktor_actor_rs::core::extension::Extension;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::transport::RemoteTransportShared;

/// Installs the endpoint supervisor and exposes [`RemotingControlHandle`].
pub struct RemotingExtensionGeneric<TB>
where
  TB: RuntimeToolbox + 'static, {
  pub(crate) control:          RemotingControlShared<TB>,
  pub(crate) transport_scheme: String,
  pub(crate) _transport:       RemoteTransportShared<TB>,
}

/// Type alias for `RemotingExtensionGeneric` with the default `NoStdToolbox`.
pub type RemotingExtension = RemotingExtensionGeneric<NoStdToolbox>;

impl<TB> RemotingExtensionGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Returns the shared control handle.
  #[must_use]
  pub fn handle(&self) -> RemotingControlShared<TB> {
    self.control.clone()
  }

  /// Returns the configured transport scheme.
  #[must_use]
  pub fn transport_scheme(&self) -> &str {
    &self.transport_scheme
  }
}

impl<TB> Extension<TB> for RemotingExtensionGeneric<TB> where TB: RuntimeToolbox + 'static {}
