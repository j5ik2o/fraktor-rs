use super::StreamError;

/// Context passed to stage logic.
pub trait StageContext<In, Out> {
  /// Requests demand from upstream.
  fn pull(&mut self);
  /// Grabs the current input element.
  fn grab(&mut self) -> In;
  /// Pushes an element downstream.
  fn push(&mut self, out: Out);
  /// Completes the stream.
  fn complete(&mut self);
  /// Fails the stream with the provided error.
  fn fail(&mut self, error: StreamError);
}
