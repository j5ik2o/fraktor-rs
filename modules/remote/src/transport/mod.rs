//! Transport abstractions for remoting.

pub mod factory;
mod loopback;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

pub use loopback::LoopbackTransport;

/// Transport abstraction that handles wire-level operations.
pub trait RemoteTransport<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Returns the canonical scheme identifier for this transport.
  fn scheme(&self) -> &str;
}
