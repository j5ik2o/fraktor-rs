//! Standard library transport factory with Tokio support.

use alloc::{boxed::Box, string::ToString};

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

#[cfg(feature = "tokio-transport")]
use super::tokio_tcp::TokioTcpTransport;
use crate::core::{LoopbackTransport, RemoteTransport, RemotingExtensionConfig, TransportError};

/// Standard library transport factory that supports both loopback and Tokio TCP.
///
/// This factory is specialized for [`StdToolbox`] because the Tokio TCP transport
/// requires standard library mutex implementations for async `Send + Sync` bounds.
pub struct StdTransportFactory;

impl StdTransportFactory {
  /// Resolves a transport instance for the provided config (std compatible).
  ///
  /// Returns a boxed transport that callers can wrap in a mutex for shared access.
  /// Uses [`StdToolbox`] for the transport's inbound handler mutex.
  pub fn build(config: &RemotingExtensionConfig) -> Result<Box<dyn RemoteTransport<StdToolbox>>, TransportError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(Box::new(LoopbackTransport::<StdToolbox>::default())),
      | "pekko.tcp" | "fraktor.tcp" => {
        #[cfg(feature = "tokio-transport")]
        {
          TokioTcpTransport::build().map(|transport| Box::new(transport) as Box<dyn RemoteTransport<StdToolbox>>)
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
