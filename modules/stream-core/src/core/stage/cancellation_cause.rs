use super::CancellationKind;

/// Carrier value for a non-failure cancellation event.
///
/// Mirrors Apache Pekko's
/// `pekko.stream.SubscriptionWithCancelException.NonFailureCancellation`
/// objects (`NoMoreElementsNeeded` and `StageWasCompleted`) by wrapping a
/// [`CancellationKind`] discriminator. The wrapper exists to keep room for
/// future contextual fields (originating stage identity, etc.) without
/// having to widen the [`CancellationKind`] enum itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationCause {
  kind: CancellationKind,
}

impl CancellationCause {
  /// Constructs a cause indicating downstream no longer requires elements.
  ///
  /// Pekko parity: `SubscriptionWithCancelException.NoMoreElementsNeeded`.
  #[must_use]
  pub const fn no_more_elements_needed() -> Self {
    Self { kind: CancellationKind::NoMoreElementsNeeded }
  }

  /// Constructs a cause indicating the upstream stage had already completed.
  ///
  /// Pekko parity: `SubscriptionWithCancelException.StageWasCompleted`.
  #[must_use]
  pub const fn stage_was_completed() -> Self {
    Self { kind: CancellationKind::StageWasCompleted }
  }

  /// Returns the discriminator describing why cancellation was issued.
  #[must_use]
  pub const fn kind(&self) -> CancellationKind {
    self.kind
  }
}
