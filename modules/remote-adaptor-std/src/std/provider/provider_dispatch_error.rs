//! Error type returned by [`crate::std::provider::StdRemoteActorRefProvider`].

use core::fmt::{Display, Formatter, Result as FmtResult};
use std::error::Error;

use fraktor_actor_core_rs::core::kernel::actor::error::ActorError;
use fraktor_remote_core_rs::core::provider::ProviderError;

/// Errors produced by [`crate::std::provider::StdRemoteActorRefProvider`].
///
/// The variant set deliberately distinguishes:
///
/// - **`NotRemote`**: a contract violation — a local `ActorPath` was passed to a watch / unwatch
///   entry point that only accepts remote paths.
/// - **`CoreProvider`**: a `ProviderError` bubbled up from `remote-core`'s pure
///   `RemoteActorRefProvider`.
/// - **`LocalProvider`**: an `ActorError` bubbled up from actor-core's local provider when the path
///   was dispatched to it.
/// - **`RemotePidExhausted`**: the adapter exhausted its synthetic pid space for remote `ActorRef`
///   values.
#[derive(Debug)]
pub enum StdRemoteActorRefProviderError {
  /// A local actor path was supplied to a remote-only entry point.
  NotRemote,
  /// `remote-core`'s `RemoteActorRefProvider` returned an error.
  CoreProvider(ProviderError),
  /// `actor-core`'s `LocalActorRefProvider` returned an error.
  LocalProvider(ActorError),
  /// The adapter exhausted its synthetic pid space for remote actor refs.
  RemotePidExhausted,
}

impl Display for StdRemoteActorRefProviderError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | StdRemoteActorRefProviderError::NotRemote => {
        f.write_str("std remote provider: local path passed to remote-only entry point")
      },
      | StdRemoteActorRefProviderError::CoreProvider(err) => {
        write!(f, "std remote provider: core provider error: {err}")
      },
      | StdRemoteActorRefProviderError::LocalProvider(err) => {
        write!(f, "std remote provider: local provider error: {err:?}")
      },
      | StdRemoteActorRefProviderError::RemotePidExhausted => {
        f.write_str("std remote provider: remote actor ref pid space exhausted")
      },
    }
  }
}

impl Error for StdRemoteActorRefProviderError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      | StdRemoteActorRefProviderError::CoreProvider(err) => Some(err),
      // `actor_core::ActorError` does not implement `Error`,
      // so we cannot return it as a source. Its details are surfaced via
      // `Display` (which uses the `Debug` representation).
      | _ => None,
    }
  }
}

impl From<ProviderError> for StdRemoteActorRefProviderError {
  fn from(err: ProviderError) -> Self {
    Self::CoreProvider(err)
  }
}

impl From<ActorError> for StdRemoteActorRefProviderError {
  fn from(err: ActorError) -> Self {
    Self::LocalProvider(err)
  }
}
