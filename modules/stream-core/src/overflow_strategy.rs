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
  /// Pekko parity: `pekko.stream.DelayOverflowStrategy.emitEarly`.
  ///
  /// In a delay stage this asks the buffer to release pending elements ahead of
  /// schedule when it would otherwise overflow. Outside of a delay stage no
  /// "early emit" exists, so this strategy falls back to the same backpressure
  /// semantics as [`Self::Backpressure`] (Pekko marks
  /// `EmitEarly.isBackpressure = true`).
  EmitEarly,
  /// Rejects the newly arrived element when the buffer is full, leaving the
  /// already-buffered elements untouched.
  ///
  /// Mirrors Pekko's `OverflowStrategy.dropNew`: the symmetric counterpart of
  /// [`Self::DropTail`] (which keeps the newest by dropping the newest
  /// _buffered_ element). Offers made while the buffer is at capacity are
  /// reported as [`crate::QueueOfferResult::Dropped`] without mutating
  /// the existing buffer contents.
  DropNew,
}
