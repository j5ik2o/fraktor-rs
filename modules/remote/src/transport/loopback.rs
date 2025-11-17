//! Placeholder loopback transport implementation used for tests.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::RemoteTransport;

/// In-process transport used for early integration testing.
pub struct LoopbackTransport;

impl LoopbackTransport {
  /// Creates a new loopback transport instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<TB: RuntimeToolbox + 'static> RemoteTransport<TB> for LoopbackTransport {
  fn scheme(&self) -> &str {
    "fraktor.loopback"
  }
}
