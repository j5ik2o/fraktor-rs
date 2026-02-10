use core::fmt;

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
    }
  }
}
