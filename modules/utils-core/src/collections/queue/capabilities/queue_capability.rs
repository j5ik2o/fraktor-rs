/// Enumerates queue capabilities required by higher-level components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueCapability {
  /// Multi-producer single-consumer MPSC semantics.
  Mpsc,
  /// Double-ended deque operations.
  Deque,
  /// Futures that wait for capacity (blocking offer/poll).
  BlockingFuture,
}
