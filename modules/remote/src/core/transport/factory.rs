//! Transport factory resolving schemes from configuration.

use alloc::string::ToString;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  loopback_transport::LoopbackTransport, remote_transport::RemoteTransport, transport_error::TransportError,
};
use crate::core::remoting_extension_config::RemotingExtensionConfig;

/// Builds transports based on configuration.
pub struct TransportFactory;

impl TransportFactory {
  /// Resolves a transport instance for the provided config.
  pub fn build(config: &RemotingExtensionConfig) -> Result<ArcShared<dyn RemoteTransport>, TransportError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(ArcShared::new(LoopbackTransport::default())),
      | scheme => Err(TransportError::UnsupportedScheme(scheme.to_string())),
    }
  }
}
