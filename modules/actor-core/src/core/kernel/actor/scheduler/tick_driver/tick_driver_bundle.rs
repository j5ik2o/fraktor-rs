//! Bundle of assets produced after provisioning a tick driver.

#[cfg(test)]
mod tests;

use core::time::Duration;

use super::{AutoDriverMetadata, TickDriverId, TickDriverKind, TickFeedHandle};

/// Bundle of assets produced after provisioning a tick driver.
pub struct TickDriverBundle {
  id:            TickDriverId,
  kind:          TickDriverKind,
  resolution:    Duration,
  feed:          Option<TickFeedHandle>,
  auto_metadata: Option<AutoDriverMetadata>,
}

impl Clone for TickDriverBundle {
  fn clone(&self) -> Self {
    Self {
      id:            self.id,
      kind:          self.kind,
      resolution:    self.resolution,
      feed:          self.feed.clone(),
      auto_metadata: self.auto_metadata.clone(),
    }
  }
}

impl TickDriverBundle {
  /// Creates a new bundle for a provisioned driver.
  #[must_use]
  pub const fn new(id: TickDriverId, kind: TickDriverKind, resolution: Duration, feed: TickFeedHandle) -> Self {
    Self { id, kind, resolution, feed: Some(feed), auto_metadata: None }
  }

  /// Creates a noop bundle (no feed) for default state.
  #[must_use]
  pub const fn noop(id: TickDriverId, kind: TickDriverKind, resolution: Duration) -> Self {
    Self { id, kind, resolution, feed: None, auto_metadata: None }
  }

  /// Annotates the bundle with auto driver metadata.
  #[must_use]
  pub const fn with_auto_metadata(mut self, metadata: AutoDriverMetadata) -> Self {
    self.auto_metadata = Some(metadata);
    self
  }

  /// Returns the unique identifier of the running driver.
  #[must_use]
  pub const fn id(&self) -> TickDriverId {
    self.id
  }

  /// Returns the kind classification of the running driver.
  #[must_use]
  pub const fn kind(&self) -> TickDriverKind {
    self.kind
  }

  /// Returns the tick resolution of the running driver.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Returns the shared tick feed handle when present.
  #[must_use]
  pub const fn feed(&self) -> Option<&TickFeedHandle> {
    self.feed.as_ref()
  }

  /// Returns the auto driver metadata if present.
  #[must_use]
  pub const fn auto_metadata(&self) -> Option<&AutoDriverMetadata> {
    self.auto_metadata.as_ref()
  }
}
