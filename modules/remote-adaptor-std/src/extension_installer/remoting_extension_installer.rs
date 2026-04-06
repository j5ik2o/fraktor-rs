//! Actor system extension installer for the new `StdRemoting` aggregate.

use std::sync::{Arc, Mutex};

use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};

use crate::{extension_installer::base::StdRemoting, tcp_transport::TcpRemoteTransport};

/// Phase B minimum-viable extension installer for the new
/// `fraktor-remote-adaptor-std-rs` runtime.
///
/// The installer constructs a [`StdRemoting`] aggregate from a transport
/// supplied by the caller and (currently) stores it in a `Mutex` so that
/// future hooks (Phase C integration tests, Phase D dependency switchover)
/// can grab the live instance.
///
/// The actor-core `ExtensionInstaller` integration mirrors the legacy
/// `modules/remote/src/std/remoting_extension_installer.rs` shape: it
/// implements `ExtensionInstaller::install` so callers can pass a
/// constructed installer into the actor system builder. The minimum-viable
/// implementation does not yet register a real extension id with
/// `install_extension_id` — that wiring waits until Section 24's
/// integration tests prove the protocol is end-to-end functional.
pub struct RemotingExtensionInstaller {
  remoting: Arc<Mutex<StdRemoting>>,
}

impl RemotingExtensionInstaller {
  /// Creates a new installer wrapping the given transport.
  ///
  /// `watcher_handle` is optional; callers that haven't started a watcher
  /// actor yet can pass `None` and install one later via
  /// [`StdRemoting::install_watcher`].
  #[must_use]
  pub fn new(transport: Arc<Mutex<TcpRemoteTransport>>) -> Self {
    Self { remoting: Arc::new(Mutex::new(StdRemoting::new(transport, None))) }
  }

  /// Returns a clone of the shared `StdRemoting` handle.
  #[must_use]
  pub fn remoting(&self) -> Arc<Mutex<StdRemoting>> {
    Arc::clone(&self.remoting)
  }
}

impl ExtensionInstaller for RemotingExtensionInstaller {
  fn install(&self, _system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    // Phase B minimum-viable: the installer is a no-op against the live
    // actor system. The aggregate is reachable via [`Self::remoting`], and
    // the integration tests in Section 24 wire it into the actor system
    // explicitly. Section 22 unblocks compile-time integration so dependent
    // crates can switch over in Phase D, even before the full extension-id
    // registration is in place.
    Ok(())
  }
}
