//! Immutable byte sequence inspired by Pekko's `ByteString`.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::sync::ArcShared;

/// An immutable, cheaply cloneable byte sequence.
///
/// Corresponds to Pekko's `org.apache.pekko.util.ByteString`.
/// Internally backed by `ArcShared<Vec<u8>>` with start/end offsets,
/// enabling zero-copy slicing and cheap cloning.
#[derive(Clone)]
pub struct ByteString {
  data:  ArcShared<Vec<u8>>,
  start: usize,
  end:   usize,
}

impl ByteString {
  /// Creates an empty `ByteString`.
  #[must_use]
  pub fn empty() -> Self {
    Self { data: ArcShared::new(Vec::new()), start: 0, end: 0 }
  }

  /// Creates a `ByteString` from a byte slice by copying.
  #[must_use]
  pub fn from_slice(bytes: &[u8]) -> Self {
    let end = bytes.len();
    Self { data: ArcShared::new(bytes.to_vec()), start: 0, end }
  }

  /// Creates a `ByteString` from a `Vec<u8>`, taking ownership.
  #[must_use]
  pub fn from_vec(bytes: Vec<u8>) -> Self {
    let end = bytes.len();
    Self { data: ArcShared::new(bytes), start: 0, end }
  }

  /// Creates a `ByteString` from a UTF-8 string.
  #[must_use]
  pub fn from_string(s: &str) -> Self {
    Self::from_slice(s.as_bytes())
  }

  /// Returns the number of bytes.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.end - self.start
  }

  /// Returns `true` if the byte string contains no bytes.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.start == self.end
  }

  /// Returns a byte slice view of the contents.
  #[must_use]
  pub fn as_slice(&self) -> &[u8] {
    &self.data[self.start..self.end]
  }

  /// Returns a raw pointer to the first byte of the viewed region.
  #[must_use]
  pub fn as_ptr(&self) -> *const u8 {
    self.as_slice().as_ptr()
  }

  /// Returns the byte at the given index, or `None` if out of bounds.
  #[must_use]
  pub fn get(&self, index: usize) -> Option<u8> {
    if index < self.len() { Some(self.data[self.start + index]) } else { None }
  }

  /// Returns a zero-copy sub-range `[from, until)`.
  ///
  /// Clamps the range to valid bounds without panicking.
  #[must_use]
  pub fn slice(&self, from: usize, until: usize) -> Self {
    let len = self.len();
    let clamped_from = from.min(len);
    let clamped_until = until.min(len).max(clamped_from);
    Self { data: self.data.clone(), start: self.start + clamped_from, end: self.start + clamped_until }
  }

  /// Returns the first `n` bytes as a zero-copy view.
  #[must_use]
  pub fn take(&self, n: usize) -> Self {
    self.slice(0, n)
  }

  /// Skips the first `n` bytes and returns the rest as a zero-copy view.
  #[must_use]
  pub fn drop_prefix(&self, n: usize) -> Self {
    self.slice(n, self.len())
  }

  /// Concatenates two `ByteString` values into a new allocation.
  #[must_use]
  pub fn concat(&self, other: &ByteString) -> Self {
    if self.is_empty() {
      return other.clone();
    }
    if other.is_empty() {
      return self.clone();
    }
    let mut buf = Vec::with_capacity(self.len() + other.len());
    buf.extend_from_slice(self.as_slice());
    buf.extend_from_slice(other.as_slice());
    Self::from_vec(buf)
  }

  /// Copies the contents into a new `Vec<u8>`.
  #[must_use]
  pub fn to_vec(&self) -> Vec<u8> {
    self.as_slice().to_vec()
  }

  /// Decodes the contents as a UTF-8 string.
  ///
  /// # Errors
  ///
  /// Returns `Err` if the bytes are not valid UTF-8.
  pub fn decode_string(&self) -> Result<String, core::str::Utf8Error> {
    core::str::from_utf8(self.as_slice()).map(String::from)
  }

  /// Returns `true` if `self` starts with the given prefix.
  #[must_use]
  pub fn starts_with(&self, prefix: &[u8]) -> bool {
    self.as_slice().starts_with(prefix)
  }

  /// Returns the index of the first occurrence of `byte`, or `None`.
  #[must_use]
  pub fn index_of(&self, byte: u8) -> Option<usize> {
    self.as_slice().iter().position(|&b| b == byte)
  }
}

impl Default for ByteString {
  fn default() -> Self {
    Self::empty()
  }
}

impl PartialEq for ByteString {
  fn eq(&self, other: &Self) -> bool {
    self.as_slice() == other.as_slice()
  }
}

impl Eq for ByteString {}

impl core::hash::Hash for ByteString {
  fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state);
  }
}

impl core::fmt::Debug for ByteString {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ByteString").field("len", &self.len()).finish()
  }
}

impl AsRef<[u8]> for ByteString {
  fn as_ref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl From<&[u8]> for ByteString {
  fn from(bytes: &[u8]) -> Self {
    Self::from_slice(bytes)
  }
}

impl From<Vec<u8>> for ByteString {
  fn from(bytes: Vec<u8>) -> Self {
    Self::from_vec(bytes)
  }
}

impl From<&str> for ByteString {
  fn from(s: &str) -> Self {
    Self::from_string(s)
  }
}

impl From<String> for ByteString {
  fn from(s: String) -> Self {
    Self::from_vec(s.into_bytes())
  }
}
