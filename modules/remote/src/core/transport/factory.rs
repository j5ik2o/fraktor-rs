//! Transport factory resolving schemes from configuration.
#![allow(cfg_std_forbid)]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

#[cfg(not(feature = "std"))]
use super::loopback_transport::LoopbackTransport;
use super::{remote_transport::RemoteTransport, transport_error::TransportError};
use crate::core::remoting_extension_config::RemotingExtensionConfig;

/// Builds transports based on configuration.
pub struct TransportFactory;

impl TransportFactory {
  /// Resolves a transport instance for the provided config.
  ///
  /// Returns a boxed transport that callers can wrap in a mutex for shared access.
  pub fn build(config: &RemotingExtensionConfig) -> Result<Box<dyn RemoteTransport>, TransportError> {
    #[cfg(feature = "std")]
    {
      // std feature が有効な場合は StdTransportFactory を使用
      crate::std::transport::StdTransportFactory::build(config)
    }

    #[cfg(not(feature = "std"))]
    {
      // no_std の場合は loopback のみサポート
      match config.transport_scheme() {
        | "fraktor.loopback" => Ok(Box::new(LoopbackTransport::default())),
        | scheme => Err(TransportError::UnsupportedScheme(scheme.to_string())),
      }
    }
  }
}
