//! Standard library transport factory with Tokio support.

use alloc::string::ToString;

use fraktor_utils_rs::core::sync::ArcShared;

#[cfg(feature = "tokio-transport")]
use super::tokio_tcp::TokioTcpTransport;
use crate::core::{LoopbackTransport, RemoteTransport, RemotingExtensionConfig, TransportError};

/// Standard library transport factory that supports both loopback and Tokio TCP.
pub struct StdTransportFactory;

impl StdTransportFactory {
  /// Resolves a transport instance for the provided config (std compatible).
  pub fn build(config: &RemotingExtensionConfig) -> Result<ArcShared<dyn RemoteTransport>, TransportError> {
    match config.transport_scheme() {
      | "fraktor.loopback" => Ok(ArcShared::new(LoopbackTransport::default())),
      | "pekko.tcp" | "fraktor.tcp" => {
        #[cfg(feature = "tokio-transport")]
        {
          TokioTcpTransport::build().map(|transport| ArcShared::new(transport) as ArcShared<dyn RemoteTransport>)
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
