//! Snapshot returned by typed topic stats queries.

/// Snapshot of a typed topic actor state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicStats {
  local_subscriber_count: usize,
  topic_instance_count:   usize,
}

impl TopicStats {
  /// Creates a new topic stats snapshot.
  #[must_use]
  pub const fn new(local_subscriber_count: usize, topic_instance_count: usize) -> Self {
    Self { local_subscriber_count, topic_instance_count }
  }

  /// Returns the number of local subscribers currently attached to this topic actor.
  #[must_use]
  pub const fn local_subscriber_count(&self) -> usize {
    self.local_subscriber_count
  }

  /// Returns the number of known topic instances for the topic.
  #[must_use]
  pub const fn topic_instance_count(&self) -> usize {
    self.topic_instance_count
  }
}
