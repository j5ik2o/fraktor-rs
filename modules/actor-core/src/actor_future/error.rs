/// Errors emitted by [`ActorFuture`].
#[derive(Debug, Eq, PartialEq)]
pub enum ActorFutureError {
  /// The future was already completed.
  AlreadyCompleted,
  /// A completion callback has already been registered.
  CallbackAlreadyRegistered,
}
