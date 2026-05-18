//! Errors returned by the [`Dispatchers`](super::Dispatchers) registry.

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors returned by [`Dispatchers`](super::dispatchers::Dispatchers) operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchersError {
  /// The identifier is already registered.
  Duplicate(String),
  /// No configurator is registered for the identifier.
  Unknown(String),
  /// An alias chain exceeded the maximum allowed depth.
  ///
  /// This indicates either a cycle in the alias graph or an excessively deep
  /// chain of aliases. Mirrors Pekko `Dispatchers.scala:160-163` which throws
  /// `ConfigurationException` once `MaxDispatcherAliasDepth = 20` is exceeded.
  AliasChainTooDeep {
    /// The identifier where alias resolution started.
    start: String,
    /// The maximum depth that was tolerated before rejection.
    depth: usize,
  },
  /// The identifier is registered as both an alias and an entry.
  ///
  /// fraktor-rs stores aliases and entries in separate maps; registering the
  /// same identifier in both would make `resolve` ambiguous. The strict
  /// `register` and `register_alias` APIs reject the conflict with this
  /// variant. `register_or_update`, on the other hand, follows last-writer-
  /// wins semantics and silently removes any pre-existing alias for the same
  /// id before inserting the entry (so the builder-style
  /// `ActorSystemConfig::with_dispatcher_factory` remains infallible).
  AliasConflictsWithEntry(String),
}

impl Display for DispatchersError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Duplicate(id) => write!(f, "dispatcher id `{id}` is already registered"),
      | Self::Unknown(id) => write!(f, "no dispatcher registered for id `{id}`"),
      | Self::AliasChainTooDeep { start, depth } => {
        write!(f, "alias chain starting at `{start}` exceeded max depth {depth} (possible cycle or excessive aliasing)")
      },
      | Self::AliasConflictsWithEntry(id) => {
        write!(f, "id `{id}` is registered as both an alias and an entry")
      },
    }
  }
}

impl core::error::Error for DispatchersError {}
