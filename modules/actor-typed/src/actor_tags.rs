//! Typed actor tag facade.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeSet, string::String};

use crate::TypedProps;

/// Pekko-compatible metadata tags for typed actor props.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActorTags {
  tags: BTreeSet<String>,
}

impl ActorTags {
  /// Creates a new tag set from the provided labels.
  #[must_use]
  pub fn new<I, S>(tags: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    Self { tags: tags.into_iter().map(Into::into).collect() }
  }

  /// Returns the stored tags.
  #[must_use]
  pub const fn tags(&self) -> &BTreeSet<String> {
    &self.tags
  }

  /// Applies the stored tags to typed props.
  #[must_use]
  pub fn apply_to<M>(self, props: TypedProps<M>) -> TypedProps<M>
  where
    M: Send + Sync + 'static, {
    props.with_tags(self.tags)
  }
}
