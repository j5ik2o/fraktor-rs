//! Event payload describing a message adapter failure.

use alloc::string::String;
use core::any::TypeId;

use crate::core::kernel::actor::Pid;

/// Event emitted when a message adapter cannot transform an incoming message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterFailureEvent {
  /// Adapter registry reached the configured capacity.
  RegistryFull {
    /// Actor that observed the failure.
    pid: Pid,
  },
  /// Adapter envelope became inconsistent while transiting the runtime.
  EnvelopeCorrupted {
    /// Actor that observed the failure.
    pid: Pid,
  },
  /// Owning actor cell was not available.
  ActorUnavailable {
    /// Actor that observed the failure.
    pid: Pid,
  },
  /// Adapter registration happened without an active registry.
  RegistryUnavailable {
    /// Actor that observed the failure.
    pid: Pid,
  },
  /// Payload type did not match the registered adapter.
  TypeMismatch {
    /// Actor that observed the failure.
    pid:     Pid,
    /// Runtime type identifier that failed to match.
    type_id: TypeId,
  },
  /// Adapter reported a domain-specific failure detail.
  Custom {
    /// Actor that observed the failure.
    pid:    Pid,
    /// Failure detail returned by the adapter.
    detail: String,
  },
}

impl AdapterFailureEvent {
  /// Creates an event for a full adapter registry.
  #[must_use]
  pub const fn registry_full(pid: Pid) -> Self {
    Self::RegistryFull { pid }
  }

  /// Creates an event for an inconsistent adapter envelope.
  #[must_use]
  pub const fn envelope_corrupted(pid: Pid) -> Self {
    Self::EnvelopeCorrupted { pid }
  }

  /// Creates an event for an unavailable owning actor.
  #[must_use]
  pub const fn actor_unavailable(pid: Pid) -> Self {
    Self::ActorUnavailable { pid }
  }

  /// Creates an event for a missing adapter registry.
  #[must_use]
  pub const fn registry_unavailable(pid: Pid) -> Self {
    Self::RegistryUnavailable { pid }
  }

  /// Creates an event for a type mismatch during adapter dispatch.
  #[must_use]
  pub const fn type_mismatch(pid: Pid, type_id: TypeId) -> Self {
    Self::TypeMismatch { pid, type_id }
  }

  /// Creates an event for an adapter-specific custom failure.
  #[must_use]
  pub fn custom(pid: Pid, detail: impl Into<String>) -> Self {
    Self::Custom { pid, detail: detail.into() }
  }

  /// Returns the actor pid that produced the adapter failure.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    match self {
      | Self::RegistryFull { pid }
      | Self::EnvelopeCorrupted { pid }
      | Self::ActorUnavailable { pid }
      | Self::RegistryUnavailable { pid }
      | Self::TypeMismatch { pid, .. }
      | Self::Custom { pid, .. } => *pid,
    }
  }
}
