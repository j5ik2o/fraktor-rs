//! Extension entry point wiring remoting control and supervisor actors.

mod config;
mod control;
mod control_backpressure_hook;
mod control_handle;
/// Bridge configuration and factory/handle traits used by transport adapters.
pub mod endpoint_bridge;
mod error;
mod lifecycle_state;
#[cfg(all(test, feature = "std"))]
mod tests;

use alloc::string::String;

pub use config::RemotingExtensionConfig;
pub use control::{RemotingControl, RemotingControlShared};
pub use control_handle::RemotingControlHandle;
pub use error::RemotingError;
use fraktor_actor_core_rs::core::kernel::actor::extension::Extension;

use crate::core::transport::RemoteTransportShared;

/// Installs the endpoint supervisor and exposes [`RemotingControlHandle`].
pub struct RemotingExtension {
  pub(crate) control:          RemotingControlShared,
  pub(crate) transport_scheme: String,
  pub(crate) _transport:       RemoteTransportShared,
}

impl RemotingExtension {
  /// Returns the shared control handle.
  #[must_use]
  pub fn handle(&self) -> RemotingControlShared {
    self.control.clone()
  }

  /// Returns the configured transport scheme.
  #[must_use]
  pub fn transport_scheme(&self) -> &str {
    &self.transport_scheme
  }
}

impl Extension for RemotingExtension {}
