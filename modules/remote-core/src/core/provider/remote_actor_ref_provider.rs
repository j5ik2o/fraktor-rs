//! Remote-only actor ref provider trait.

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_path::ActorPath};

use crate::core::provider::{provider_error::ProviderError, remote_actor_ref::RemoteActorRef};

/// The single remote-only actor ref provider port.
///
/// `RemoteActorRefProvider` is scoped to **remote path resolution**; it does
/// not know anything about local actor paths and never returns an actor-core
/// `ActorRef`. This is an intentional split driven by design Decision 3-C:
/// the adapter layer (Phase B) is responsible for inspecting an `ActorPath`,
/// deciding whether it addresses a local or remote actor, and dispatching to
/// the appropriate provider (local → actor-core local provider, remote →
/// this trait).
///
/// Implementations live in `fraktor-remote-adaptor-std-rs` (Phase B).
pub trait RemoteActorRefProvider {
  /// Resolves a **remote** [`ActorPath`] into a [`RemoteActorRef`].
  ///
  /// # Contract
  ///
  /// - Callers MUST pass a path whose authority refers to a remote node. Passing a local path (one
  ///   whose authority matches the local `UniqueAddress`) is a contract violation and
  ///   implementations SHOULD return [`ProviderError::NotRemote`].
  /// - Implementations MAY maintain internal caches (e.g. authority → remote node id) and therefore
  ///   take `&mut self`; this is a deliberate CQS exception motivated by the common Pekko-style
  ///   caching that `RemoteActorRefProvider.actorFor` performs.
  /// - The local / remote dispatch is the **adapter**'s responsibility: the
  ///   `fraktor-remote-adaptor-std-rs` provider implementation inspects each `ActorPath` via
  ///   [`crate::core::provider::resolve_remote_address`] and either forwards to the actor-core
  ///   local provider (for local paths) or to this method (for remote paths).
  ///
  /// # Errors
  ///
  /// Returns a [`ProviderError`] variant describing why the path could not be
  /// resolved into a [`RemoteActorRef`].
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError>;

  /// Registers a death-watch between `watcher` and the remote `watchee`.
  ///
  /// Actual heartbeat delivery and tick-driven evaluation happen in
  /// `fraktor-remote-adaptor-std-rs::watcher_actor/` (Phase B); this trait
  /// method only declares the intent and updates any internal watch state
  /// the provider might keep.
  ///
  /// # Errors
  ///
  /// Returns [`ProviderError::NotRemote`] if `watchee` addresses a local
  /// actor (the local watcher must be used instead).
  fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>;

  /// Cancels a previously registered death-watch.
  ///
  /// # Errors
  ///
  /// Returns [`ProviderError::NotRemote`] if `watchee` addresses a local
  /// actor. Mirrors [`RemoteActorRefProvider::watch`].
  fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError>;
}
