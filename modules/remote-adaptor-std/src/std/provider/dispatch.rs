//! `StdRemoteActorRefProvider` — adapter that dispatches an `ActorPath` to
//! either actor-core's local provider or `remote-core`'s remote provider per
//! design Decision 3-C.

use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid,
  actor_path::ActorPath,
  actor_ref::ActorRef,
  actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider},
};
use fraktor_remote_core_rs::domain::{
  address::UniqueAddress,
  provider::{RemoteActorRefProvider, resolve_remote_address},
};
use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::std::{
  provider::provider_dispatch_error::StdRemoteActorRefProviderError, tcp_transport::TcpRemoteTransport,
};

/// `std + tokio` actor ref provider that performs the loopback / remote
/// dispatch demanded by design Decision 3-C.
///
/// `StdRemoteActorRefProvider::actor_ref` follows the three-branch dispatch
/// rule from the spec:
///
/// 1. **No authority** (`authority_endpoint().is_none()`) → forward to the local provider
///    unchanged.
/// 2. **Authority matches the local one** (host/port/system equal, with `uid == 0` treated as a
///    wildcard per design Decision 13) → strip the authority and forward the local-equivalent path
///    to the local provider.
/// 3. **Authority does not match** → resolve through the core remote provider, build a
///    [`crate::std::provider::RemoteActorRefSender`], and wrap it as an `ActorRef`.
///
/// `watch` and `unwatch` are remote-only — local death-watch is handled by
/// actor-core's normal `ActorContext::watch` path on the resolved local
/// `ActorRef`.
pub struct StdRemoteActorRefProvider {
  local_address:   UniqueAddress,
  local_provider:  ActorRefProviderHandleShared<LocalActorRefProvider>,
  remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
  transport:       SharedLock<TcpRemoteTransport>,
}

impl StdRemoteActorRefProvider {
  /// Creates a new dispatcher.
  #[must_use]
  pub fn new(
    local_address: UniqueAddress,
    local_provider: ActorRefProviderHandleShared<LocalActorRefProvider>,
    remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
    transport: SharedLock<TcpRemoteTransport>,
  ) -> Self {
    Self { local_address, local_provider, remote_provider, transport }
  }

  /// Returns the local [`UniqueAddress`] used to determine the loopback
  /// branch.
  #[must_use]
  pub const fn local_address(&self) -> &UniqueAddress {
    &self.local_address
  }

  /// Resolves an [`ActorPath`] into an actor-core [`ActorRef`].
  ///
  /// # Errors
  ///
  /// Returns [`StdRemoteActorRefProviderError`] when:
  ///
  /// - the local provider rejects the path (`LocalProvider`),
  /// - the core remote provider rejects the path (`CoreProvider`), or
  /// - the resolved [`fraktor_remote_core_rs::domain::provider::RemoteActorRef`] cannot be wrapped
  ///   into an `ActorRef` (`RemoteSenderBuildFailed`).
  ///
  /// # Panics
  ///
  /// Phase B minimum-viable: a non-loopback remote `actor_ref` returns
  /// `RemoteSenderBuildFailed` because constructing a real
  /// `ActorRef` requires extra system context (a live `ActorSystemState`)
  /// that is wired up in Section 22's `StdRemoting` extension installer.
  pub fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, StdRemoteActorRefProviderError> {
    if path.parts().authority_endpoint().is_none() {
      // Branch 1: no authority → straight to the local provider.
      return self.local_provider.actor_ref(path).map_err(StdRemoteActorRefProviderError::from);
    }
    if let Some(resolved) = resolve_remote_address(&path)
      && self.is_local_authority(&resolved)
    {
      // Branch 2: authority matches the local node → strip the authority
      // and forward the local-equivalent path to the local provider.
      let local_path = strip_authority(path);
      return self.local_provider.actor_ref(local_path).map_err(StdRemoteActorRefProviderError::from);
    }
    // Branch 3: authority does not match → core remote provider.
    // Resolving the path validates remote routing; constructing the
    // ActorRef wrapper requires actor system context wired in Section 22.
    self.remote_provider.actor_ref(path).map_err(StdRemoteActorRefProviderError::from)?;
    Err(StdRemoteActorRefProviderError::RemoteSenderBuildFailed)
  }

  /// Registers a remote death-watch.
  ///
  /// # Errors
  ///
  /// Returns [`StdRemoteActorRefProviderError::NotRemote`] if `watchee` is a
  /// local actor path. Local death-watch must be performed via the normal
  /// actor-core `ActorContext::watch` path on the resolved local
  /// [`ActorRef`].
  pub fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), StdRemoteActorRefProviderError> {
    if !self.is_remote_path(&watchee) {
      return Err(StdRemoteActorRefProviderError::NotRemote);
    }
    self.remote_provider.watch(watchee, watcher).map_err(StdRemoteActorRefProviderError::from)
  }

  /// Cancels a previously registered remote death-watch.
  ///
  /// # Errors
  ///
  /// Mirrors [`Self::watch`] — local watchees return
  /// [`StdRemoteActorRefProviderError::NotRemote`].
  pub fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), StdRemoteActorRefProviderError> {
    if !self.is_remote_path(&watchee) {
      return Err(StdRemoteActorRefProviderError::NotRemote);
    }
    self.remote_provider.unwatch(watchee, watcher).map_err(StdRemoteActorRefProviderError::from)
  }

  /// Returns the underlying transport handle.
  ///
  /// Exposed for the `extension_installer` (Section 22) so it can wire the
  /// transport into other adapter components without going through this
  /// type's mutable methods.
  #[must_use]
  pub fn transport(&self) -> SharedLock<TcpRemoteTransport> {
    self.transport.clone()
  }

  fn is_local_authority(&self, resolved: &UniqueAddress) -> bool {
    if resolved.address() != self.local_address.address() {
      return false;
    }
    // uid == 0 acts as a wildcard per design Decision 13.
    resolved.uid() == 0 || resolved.uid() == self.local_address.uid()
  }

  fn is_remote_path(&self, path: &ActorPath) -> bool {
    let Some(resolved) = resolve_remote_address(path) else {
      return false;
    };
    !self.is_local_authority(&resolved)
  }
}

/// Returns a copy of `path` with its authority component stripped.
///
/// Authority-less paths are unchanged.
fn strip_authority(path: ActorPath) -> ActorPath {
  use fraktor_actor_core_rs::core::kernel::actor::actor_path::ActorPathParser;

  // Phase B minimum-viable: re-parse the path's relative form. The actor-core
  // `ActorPath` does not currently expose a `with_authority(None)` builder so
  // we go through the parser. The cost is acceptable for the slow path
  // (loopback dispatch) and the conversion is lossless because the relative
  // string contains every segment that the local provider needs.
  let relative = path.to_relative_string();
  ActorPathParser::parse(&relative).unwrap_or(path)
}
