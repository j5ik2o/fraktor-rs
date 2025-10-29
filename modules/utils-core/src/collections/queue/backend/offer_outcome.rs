/// Outcome produced by a queue offer operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferOutcome {
  /// The element was enqueued without any side effects.
  Enqueued,
  /// The offer succeeded after evicting the oldest items.
  DroppedOldest {
    /// Number of elements removed from the head of the queue.
    count: usize,
  },
  /// The offer succeeded after discarding the newest items.
  DroppedNewest {
    /// Number of offered elements dropped without enqueuing.
    count: usize,
  },
  /// The underlying storage grew to the specified capacity.
  GrewTo {
    /// New capacity after the storage has grown.
    capacity: usize,
  },
}

impl From<&OfferOutcome> for &'static str {
  fn from(outcome: &OfferOutcome) -> Self {
    match outcome {
      | OfferOutcome::Enqueued => "enqueue",
      | OfferOutcome::DroppedOldest { .. } => "drop_oldest",
      | OfferOutcome::DroppedNewest { .. } => "drop_newest",
      | OfferOutcome::GrewTo { .. } => "grow",
    }
  }
}
