//! Transport factory resolving schemes from configuration.
#![allow(cfg_std_forbid)]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

#[cfg(not(feature = "std"))]
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;
#[cfg(feature = "std")]
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

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
  /// Uses [`StdToolbox`] for the transport's inbound handler mutex.
  #[cfg(feature = "std")]
  pub fn build(config: &RemotingExtensionConfig) -> Result<Box<dyn RemoteTransport<StdToolbox>>, TransportError> {
    crate::std::transport::StdTransportFactory::build(config)
  }

  /// Resolves a transport instance for the provided config (no_std version).
  ///
  /// Returns a boxed transport that callers can wrap in a mutex for shared access.
  #[cfg(not(feature = "std"))]
  pub fn build<TB: RuntimeToolbox + 'static>(
    config: &RemotingExtensionConfig,
  ) -> Result<Box<dyn RemoteTransport<TB>>, TransportError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(Box::new(LoopbackTransport::<TB>::default())),
      | scheme => Err(TransportError::UnsupportedScheme(scheme.to_string())),
    }
  }
}
