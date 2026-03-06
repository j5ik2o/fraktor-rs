#[cfg(test)]
mod tests;

/// Overflow strategy names aligned with Pekko stream APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
  /// Propagates backpressure to upstream when the buffer is full.
  Backpressure,
  /// Drops the oldest buffered element and enqueues the newest.
  DropHead,
  /// Drops the newest buffered element and keeps older buffered elements.
  DropTail,
  /// Clears the buffer and keeps only the newest offered element.
  DropBuffer,
  /// Fails the stream when the buffer is full.
  Fail,
}
