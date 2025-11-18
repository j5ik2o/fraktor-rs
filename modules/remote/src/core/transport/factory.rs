//! Transport factory resolving schemes from configuration.
#![allow(cfg_std_forbid)]
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use fraktor_utils_rs::core::sync::ArcShared;

#[cfg(not(feature = "std"))]
use super::loopback_transport::LoopbackTransport;
use super::{remote_transport::RemoteTransport, transport_error::TransportError};
use crate::core::remoting_extension_config::RemotingExtensionConfig;

/// Builds transports based on configuration.
pub struct TransportFactory;

impl TransportFactory {
  /// Resolves a transport instance for the provided config.
  pub fn build(config: &RemotingExtensionConfig) -> Result<ArcShared<dyn RemoteTransport>, TransportError> {
    #[cfg(feature = "std")]
    {
      // std feature が有効な場合は StdTransportFactory を使用
      crate::std::transport::StdTransportFactory::build(config)
    }

    #[cfg(not(feature = "std"))]
    {
      // no_std の場合は loopback のみサポート
      match config.transport_scheme() {
        | "fraktor.loopback" => Ok(ArcShared::new(LoopbackTransport::default())),
        | scheme => Err(TransportError::UnsupportedScheme(scheme.to_string())),
      }
    }
  }
}
