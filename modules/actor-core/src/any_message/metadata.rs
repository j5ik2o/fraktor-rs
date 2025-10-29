use alloc::{borrow::Cow, vec::Vec};
use core::iter::FusedIterator;

/// Metadata entries attached to a message.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageMetadata {
  entries: Vec<MetadataEntry>,
}

impl MessageMetadata {
  /// Creates an empty metadata collection.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: Vec::new() }
  }

  /// Returns `true` when the collection contains no entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Returns the number of stored entries.
  #[must_use]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Inserts or replaces a key-value pair.
  pub fn insert(&mut self, key: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) {
    let key = key.into();
    let value = value.into();

    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.value = value;
      return;
    }

    self.entries.push(MetadataEntry { key, value });
  }

  /// Retrieves a value by key.
  #[must_use]
  pub fn get(&self, key: &str) -> Option<&str> {
    self.entries.iter().find(|entry| entry.key == key).map(|entry| entry.value.as_ref())
  }

  /// Returns an iterator over key-value pairs.
  pub fn iter(&self) -> MessageMetadataIter<'_> {
    MessageMetadataIter { inner: self.entries.iter() }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataEntry {
  key:   Cow<'static, str>,
  value: Cow<'static, str>,
}

/// Iterator over metadata entries.
pub struct MessageMetadataIter<'a> {
  inner: core::slice::Iter<'a, MetadataEntry>,
}

impl<'a> Iterator for MessageMetadataIter<'a> {
  type Item = (&'a str, &'a str);

  fn next(&mut self) -> Option<Self::Item> {
    self.inner.next().map(|entry| (entry.key.as_ref(), entry.value.as_ref()))
  }
}

impl<'a> FusedIterator for MessageMetadataIter<'a> {}
