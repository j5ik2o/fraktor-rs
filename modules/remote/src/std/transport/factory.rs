//! Standard library transport factory with Tokio support.

use alloc::{boxed::Box, string::ToString};

#[cfg(feature = "tokio-transport")]
use super::tokio_tcp::TokioTcpTransport;
use crate::core::{
  remoting_extension::RemotingExtensionConfig,
  transport::{LoopbackTransport, RemoteTransport, TransportError},
};

/// Standard library transport factory that supports both loopback and Tokio TCP.
///
/// This factory supports both loopback and Tokio TCP transport creation.
pub struct StdTransportFactory;

impl StdTransportFactory {
  /// Resolves a transport instance for the provided config (std compatible).
  ///
  /// Returns a boxed transport that callers can wrap in a mutex for shared access.
  /// The returned transport can be wrapped in a shared handle for concurrent use.
  pub fn build(config: &RemotingExtensionConfig) -> Result<Box<dyn RemoteTransport>, TransportError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(Box::new(LoopbackTransport::default())),
      | "pekko.tcp" | "fraktor.tcp" => {
        #[cfg(feature = "tokio-transport")]
        {
          TokioTcpTransport::build().map(|transport| Box::new(transport) as Box<dyn RemoteTransport>)
        }
        #[cfg(not(feature = "tokio-transport"))]
        {
          Err(TransportError::UnsupportedScheme(config.transport_scheme().to_string()))
        }
      },
      | scheme => Err(TransportError::UnsupportedScheme(scheme.to_string())),
    }
  }
}
