#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, format};
use core::{any::TypeId, fmt};

use fraktor_actor_rs::core::kernel::error::SendError;

/// Errors returned by stream operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
  /// Indicates an invalid demand request.
  InvalidDemand {
    /// Requested demand amount.
    requested: u64,
  },
  /// Indicates demand consumption exceeded the remaining demand.
  DemandExceeded {
    /// Requested demand amount.
    requested: u64,
    /// Remaining demand amount.
    remaining: u64,
  },
  /// Indicates a queue overflow or backpressure failure.
  BufferOverflow,
  /// Indicates the materializer has not been started.
  MaterializerNotStarted,
  /// Indicates the materializer has already been started.
  MaterializerAlreadyStarted,
  /// Indicates the materializer has already been stopped.
  MaterializerStopped,
  /// Indicates an actor system is missing for actor-backed materializers.
  ActorSystemMissing,
  /// Indicates a stream graph connection is invalid.
  InvalidConnection,
  /// Indicates a type mismatch at runtime.
  TypeMismatch,
  /// Indicates that processing cannot make progress yet and should be retried.
  WouldBlock,
  /// Indicates stream processing failed with a user error.
  Failed,
  /// Indicates stream processing failed with a preserved error context.
  FailedWithContext {
    /// Human-readable failure message.
    message:        Cow<'static, str>,
    /// Type identity of the original error, if recorded.
    source_type_id: Option<TypeId>,
  },
  /// Indicates compression/decompression failed.
  CompressionError {
    /// Compression error kind identifier.
    kind: &'static str,
  },
  /// Indicates that a partition route is invalid.
  InvalidRoute {
    /// Route value returned by a partitioner.
    route:           isize,
    /// Total partition count.
    partition_count: usize,
  },
  /// Indicates that observed substream keys exceeded the configured limit.
  SubstreamLimitExceeded {
    /// Maximum allowed substream count.
    max_substreams: usize,
  },
  /// Indicates that a timeout condition was reached.
  Timeout {
    /// Timeout kind identifier.
    kind:  &'static str,
    /// Configured tick threshold.
    ticks: u64,
  },
  /// Downstream canceled without triggering lazy source materialization.
  NeverMaterialized,
  /// Stream is terminated. Materialized value is detached.
  StreamDetached,
  /// Indicates an IO operation failed.
  IoError {
    /// IO error kind identifier (e.g. `"BrokenPipe"`, `"UnexpectedEof"`).
    kind:    alloc::string::String,
    /// Human-readable description of the error.
    message: alloc::string::String,
  },
}

impl StreamError {
  /// Creates a failed stream error while preserving a human-readable context.
  #[must_use]
  pub fn failed_with_context(message: impl Into<Cow<'static, str>>) -> Self {
    Self::FailedWithContext { message: message.into(), source_type_id: None }
  }

  /// Creates a failed stream error tagged with the source error type.
  #[must_use]
  pub fn failed_typed<E: 'static>(message: impl Into<Cow<'static, str>>) -> Self {
    Self::FailedWithContext { message: message.into(), source_type_id: Some(TypeId::of::<E>()) }
  }

  /// Returns the type identity of the original error when recorded.
  #[must_use]
  pub const fn source_type_id(&self) -> Option<TypeId> {
    match self {
      | Self::FailedWithContext { source_type_id, .. } => *source_type_id,
      | _ => None,
    }
  }

  /// Returns `true` when the stored source error matches the provided type.
  #[must_use]
  pub fn is_source_type<E: 'static>(&self) -> bool {
    self.source_type_id() == Some(TypeId::of::<E>())
  }

  /// Creates a stream error from a send failure while preserving send context.
  #[must_use]
  pub fn from_send_error(error: &SendError) -> Self {
    match error {
      | SendError::Full(_) | SendError::Suspended(_) => Self::WouldBlock,
      | SendError::Closed(_) | SendError::NoRecipient(_) | SendError::Timeout(_) => {
        Self::failed_typed::<SendError>(format!("send failed: {error:?}"))
      },
    }
  }
}

impl fmt::Display for StreamError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::InvalidDemand { requested } => write!(f, "invalid demand: {requested}"),
      | Self::DemandExceeded { requested, remaining } => {
        write!(f, "demand exceeded: requested={requested} remaining={remaining}")
      },
      | Self::BufferOverflow => write!(f, "stream buffer overflow"),
      | Self::MaterializerNotStarted => write!(f, "materializer not started"),
      | Self::MaterializerAlreadyStarted => write!(f, "materializer already started"),
      | Self::MaterializerStopped => write!(f, "materializer stopped"),
      | Self::ActorSystemMissing => write!(f, "actor system missing"),
      | Self::InvalidConnection => write!(f, "invalid stream connection"),
      | Self::TypeMismatch => write!(f, "stream type mismatch"),
      | Self::WouldBlock => write!(f, "stream would block"),
      | Self::Failed => write!(f, "stream failed"),
      | Self::FailedWithContext { message, .. } => write!(f, "{message}"),
      | Self::CompressionError { kind } => write!(f, "compression error: {kind}"),
      | Self::InvalidRoute { route, partition_count } => {
        write!(f, "invalid partition route: route={route} partition_count={partition_count}")
      },
      | Self::SubstreamLimitExceeded { max_substreams } => {
        write!(f, "substream limit exceeded: max_substreams={max_substreams}")
      },
      | Self::Timeout { kind, ticks } => {
        write!(f, "{kind} timeout after {ticks} ticks")
      },
      | Self::NeverMaterialized => {
        write!(f, "downstream canceled without triggering lazy source materialization")
      },
      | Self::StreamDetached => {
        write!(f, "stream is terminated, materialized value is detached")
      },
      | Self::IoError { kind, message } => {
        write!(f, "IO error ({kind}): {message}")
      },
    }
  }
}
