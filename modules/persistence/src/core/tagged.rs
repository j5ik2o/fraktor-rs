//! Tagged event payload for query-oriented indexing.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeSet, string::String};
use core::{
  any::Any,
  fmt::{Debug, Formatter},
};

use fraktor_utils_rs::core::sync::ArcShared;

/// Event payload with a set of query tags.
pub struct Tagged {
  payload: ArcShared<dyn Any + Send + Sync>,
  tags:    BTreeSet<String>,
}

impl Tagged {
  /// Creates a tagged payload from an existing tag set.
  #[must_use]
  pub fn new(payload: ArcShared<dyn Any + Send + Sync>, tags: BTreeSet<String>) -> Self {
    Self { payload, tags }
  }

  /// Creates a tagged payload from an iterator of tag values.
  #[must_use]
  pub fn with_tags<I, S>(payload: ArcShared<dyn Any + Send + Sync>, tags: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    let normalized_tags = tags.into_iter().map(Into::into).collect();
    Self::new(payload, normalized_tags)
  }

  /// Returns the tagged payload.
  #[must_use]
  pub fn payload(&self) -> &ArcShared<dyn Any + Send + Sync> {
    &self.payload
  }

  /// Returns all tags attached to this payload.
  #[must_use]
  pub const fn tags(&self) -> &BTreeSet<String> {
    &self.tags
  }

  /// Returns whether the tagged payload contains the requested tag.
  #[must_use]
  pub fn contains_tag(&self, tag: &str) -> bool {
    self.tags.contains(tag)
  }

  /// Attempts to downcast the payload to the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.payload.downcast_ref::<T>()
  }
}

impl Debug for Tagged {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Tagged").field("tags", &self.tags).field("payload", &"<any>").finish()
  }
}
