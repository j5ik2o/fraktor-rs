/// Reason classification for non-failure stream cancellations.
///
/// Mirrors Apache Pekko's
/// `pekko.stream.SubscriptionWithCancelException.NonFailureCancellation`
/// sealed hierarchy (`NoMoreElementsNeeded` and `StageWasCompleted`),
/// preserving the source/sink-side semantic distinction without resorting
/// to an exception hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationKind {
  /// Downstream signaled it no longer needs further elements (sink-driven cancel).
  NoMoreElementsNeeded,
  /// The originating stage already completed before consuming further demand.
  StageWasCompleted,
}
