//! Factory responsible for instantiating transport implementations.

use alloc::borrow::ToOwned;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::{RemotingError, RemotingExtensionConfig};

use super::{LoopbackTransport, RemoteTransport};

/// Factory for creating transport instances based on configuration.
pub struct TransportFactory;

impl TransportFactory {
  /// Creates the transport implementation for the configured scheme.
  pub fn create<TB: RuntimeToolbox + 'static>(
    config: &RemotingExtensionConfig,
  ) -> Result<ArcShared<dyn RemoteTransport<TB>>, RemotingError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(ArcShared::new(LoopbackTransport::new())),
      | other => Err(RemotingError::TransportUnavailable(other.to_owned())),
    }
  }
}
