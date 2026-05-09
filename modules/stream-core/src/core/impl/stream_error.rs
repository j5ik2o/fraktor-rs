#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, boxed::Box, format, string::String};
use core::{
  any::TypeId,
  fmt::{self, Formatter, Result as FmtResult},
};

use fraktor_actor_core_rs::actor::error::SendError;

use super::FramingErrorKind;
use crate::core::stage::{CancellationCause, CancellationKind};

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
  ///
  /// Pekko parity: `pekko.stream.BufferOverflowException`.
  BufferOverflow,
  /// Indicates the materializer has not been started.
  MaterializerNotStarted,
  /// Indicates the materializer has already been started.
  MaterializerAlreadyStarted,
  /// Indicates the materializer has already been stopped.
  MaterializerStopped,
  /// Indicates an actor system is missing for actor-backed materializers.
  ActorSystemMissing,
  /// Indicates that a stage actor was read before it was initialized.
  ///
  /// Pekko parity: `StageActorRefNotInitializedException`.
  StageActorRefNotInitialized,
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
  /// Indicates that materialization failed and rollback also failed.
  MaterializedResourceRollbackFailed {
    /// Original materialization failure.
    primary: Box<StreamError>,
    /// Failure observed while rolling back resources.
    cleanup: Box<StreamError>,
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
  ///
  /// Pekko parity: `pekko.stream.TooManySubstreamsOpenException`.
  /// The Pekko exception carries no payload, but fraktor-rs retains
  /// `max_substreams` for Debug/diagnostic purposes (not surfaced by
  /// [`fmt::Display`]).
  TooManySubstreamsOpen {
    /// Maximum allowed substream count (preserved for diagnostics only).
    max_substreams: usize,
  },
  /// Indicates that a timeout condition was reached.
  ///
  /// Pekko parity mapping for `kind`:
  /// - `"initial"` → `InitialTimeoutException`
  /// - `"completion"` → `CompletionTimeoutException`
  /// - `"idle"` → `StreamIdleTimeoutException`
  /// - `"backpressure"` → `BackpressureTimeoutException`
  Timeout {
    /// Timeout kind identifier (see Pekko parity mapping above).
    kind:  &'static str,
    /// Configured tick threshold.
    ticks: u64,
  },
  /// Downstream canceled without triggering lazy source materialization.
  NeverMaterialized,
  /// Stream is terminated. Materialized value is detached.
  ///
  /// Pekko parity: `pekko.stream.StreamDetachedException`.
  StreamDetached,
  /// Indicates that a StreamRef target actor reference was used before initialization.
  ///
  /// Pekko parity: `pekko.stream.TargetRefNotInitializedYetException`.
  StreamRefTargetNotInitialized,
  /// Indicates that the remote side did not subscribe to a StreamRef in time.
  ///
  /// Pekko parity: `pekko.stream.StreamRefSubscriptionTimeoutException`.
  StreamRefSubscriptionTimeout {
    /// Human-readable timeout message.
    message: Cow<'static, str>,
  },
  /// Indicates that the remote StreamRef actor terminated.
  ///
  /// Pekko parity: `pekko.stream.RemoteStreamRefActorTerminatedException`.
  RemoteStreamRefActorTerminated {
    /// Human-readable termination message.
    message: Cow<'static, str>,
  },
  /// Indicates that a StreamRef sequence number is not the expected one.
  ///
  /// Pekko parity: `pekko.stream.InvalidSequenceNumberException`.
  InvalidSequenceNumber {
    /// Expected sequence number.
    expected_seq_nr: u64,
    /// Received sequence number.
    got_seq_nr:      u64,
    /// Human-readable sequence failure message.
    message:         Cow<'static, str>,
  },
  /// Indicates that a StreamRef message came from a non-partner actor.
  ///
  /// Pekko parity: `pekko.stream.InvalidPartnerActorException`.
  InvalidPartnerActor {
    /// Expected partner actor reference.
    expected_ref: Cow<'static, str>,
    /// Received actor reference.
    got_ref:      Cow<'static, str>,
    /// Human-readable partner failure message.
    message:      Cow<'static, str>,
  },
  /// Indicates an IO operation failed.
  IoError {
    /// IO error kind identifier (e.g. `"BrokenPipe"`, `"UnexpectedEof"`).
    kind:    String,
    /// Human-readable description of the error.
    message: String,
  },
  /// Indicates that a configured stream element limit has been reached.
  ///
  /// Pekko parity: `pekko.stream.StreamLimitReachedException(n)`.
  StreamLimitReached {
    /// Limit value (`n`) that was reached.
    limit: u64,
  },
  /// Indicates that an actor watched by a stream stage has terminated.
  ///
  /// Pekko parity: `pekko.stream.WatchedActorTerminatedException`.
  WatchedActorTerminated {
    /// Stage name that was watching the actor (e.g. `"ask"`, `"watch"`).
    watching_stage_name: Cow<'static, str>,
    /// Address/path of the actor that terminated.
    actor_path:          Cow<'static, str>,
  },
  /// Indicates that a stream terminated abruptly (e.g. materializer shutdown).
  ///
  /// Pekko parity: `pekko.stream.AbruptStreamTerminationException`.
  AbruptStreamTermination {
    /// Human-readable description of the termination context.
    message: Cow<'static, str>,
  },
  /// Indicates that an individual graph stage was terminated abruptly.
  ///
  /// Pekko parity: `pekko.stream.AbruptStageTerminationException`.
  AbruptStageTermination {
    /// Stage logic name that was terminated.
    stage_name: Cow<'static, str>,
  },
  /// Indicates a framing failure when decoding delimited byte streams.
  ///
  /// Pekko parity: `pekko.stream.scaladsl.Framing.FramingException`. The inner
  /// [`FramingErrorKind`] discriminates the specific cause (oversized frame
  /// versus malformed byte sequence) without requiring string parsing.
  Framing {
    /// Inner sub-classification of the framing failure.
    kind: FramingErrorKind,
  },
  /// Indicates a non-failure cancellation event tagged with its cause.
  ///
  /// Pekko parity: `pekko.stream.SubscriptionWithCancelException.NonFailureCancellation`.
  CancellationCause {
    /// Cause discriminator distinguishing the cancellation reason.
    cause: CancellationCause,
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

  /// Creates a rollback failure that keeps both the primary and cleanup errors.
  #[must_use]
  pub fn materialized_resource_rollback_failed(primary: Self, cleanup: Self) -> Self {
    Self::MaterializedResourceRollbackFailed { primary: Box::new(primary), cleanup: Box::new(cleanup) }
  }

  /// Returns the original materialization failure if this error represents rollback failure.
  #[must_use]
  pub const fn materialization_primary_failure(&self) -> Option<&Self> {
    match self {
      | Self::MaterializedResourceRollbackFailed { primary, .. } => Some(primary),
      | _ => None,
    }
  }

  /// Returns the cleanup failure if this error represents rollback failure.
  #[must_use]
  pub const fn materialization_cleanup_failure(&self) -> Option<&Self> {
    match self {
      | Self::MaterializedResourceRollbackFailed { cleanup, .. } => Some(cleanup),
      | _ => None,
    }
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
      | SendError::Closed(_) | SendError::NoRecipient(_) | SendError::Timeout(_) | SendError::InvalidPayload { .. } => {
        Self::failed_typed::<SendError>(format!("send failed: {error:?}"))
      },
    }
  }
}

impl fmt::Display for StreamError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
      | Self::StageActorRefNotInitialized => {
        write!(f, "You must first call getStageActor, to initialize the Actors behavior")
      },
      | Self::InvalidConnection => write!(f, "invalid stream connection"),
      | Self::TypeMismatch => write!(f, "stream type mismatch"),
      | Self::WouldBlock => write!(f, "stream would block"),
      | Self::Failed => write!(f, "stream failed"),
      | Self::FailedWithContext { message, .. } => write!(f, "{message}"),
      | Self::MaterializedResourceRollbackFailed { primary, cleanup } => {
        write!(f, "materialization failed: {primary}; rollback failed: {cleanup}")
      },
      | Self::CompressionError { kind } => write!(f, "compression error: {kind}"),
      | Self::InvalidRoute { route, partition_count } => {
        write!(f, "invalid partition route: route={route} partition_count={partition_count}")
      },
      | Self::TooManySubstreamsOpen { .. } => {
        write!(f, "Cannot open a new substream as there are too many substreams open")
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
      | Self::StreamRefTargetNotInitialized => {
        write!(
          f,
          "Internal remote target actor ref not yet resolved, yet attempted to send messages to it. \
           This should not happen due to proper flow-control."
        )
      },
      | Self::StreamRefSubscriptionTimeout { message } => write!(f, "{message}"),
      | Self::RemoteStreamRefActorTerminated { message } => write!(f, "{message}"),
      | Self::InvalidSequenceNumber { expected_seq_nr, got_seq_nr, message } => {
        write!(
          f,
          "{message} (expected: {expected_seq_nr}, got: {got_seq_nr}). \
           In most cases this means that message loss on this connection has occurred and the stream will fail eagerly."
        )
      },
      | Self::InvalidPartnerActor { expected_ref, got_ref, message } => {
        write!(
          f,
          "{message} (expected: {expected_ref}, got: {got_ref}). \
           This may happen due to 'double-materialization' on the other side of this stream ref. \
           Do note that stream refs are one-shot references and have to be paired up in 1:1 pairs. \
           Multi-cast such as broadcast etc can be implemented by sharing multiple new stream references."
        )
      },
      | Self::IoError { kind, message } => {
        write!(f, "IO error ({kind}): {message}")
      },
      | Self::StreamLimitReached { limit } => write!(f, "limit of {limit} reached"),
      | Self::WatchedActorTerminated { watching_stage_name, actor_path } => {
        write!(f, "Actor watched by [{watching_stage_name}] has terminated! Was: {actor_path}")
      },
      | Self::AbruptStreamTermination { message } => write!(f, "{message}"),
      | Self::AbruptStageTermination { stage_name } => {
        write!(
          f,
          "GraphStage [{stage_name}] terminated abruptly, caused by for example materializer or actor system termination."
        )
      },
      | Self::Framing { kind } => write!(f, "{kind}"),
      | Self::CancellationCause { cause } => match cause.kind() {
        | CancellationKind::NoMoreElementsNeeded => write!(f, "NoMoreElementsNeeded"),
        | CancellationKind::StageWasCompleted => write!(f, "StageWasCompleted"),
      },
    }
  }
}
