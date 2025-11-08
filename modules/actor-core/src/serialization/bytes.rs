use alloc::vec::Vec;
use core::{fmt, ops::Deref};

use serde::{Deserialize, Serialize};

/// Owned binary buffer used by serialized payloads.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Bytes {
  inner: Vec<u8>,
}

impl Bytes {
  /// Creates a new empty buffer.
  #[must_use]
  pub const fn new() -> Self {
    Self { inner: Vec::new() }
  }

  /// Creates a buffer from the provided vector.
  #[must_use]
  pub const fn from_vec(inner: Vec<u8>) -> Self {
    Self { inner }
  }

  /// Consumes the buffer and returns the underlying vector.
  #[must_use]
  pub fn into_vec(self) -> Vec<u8> {
    self.inner
  }

  /// Returns the number of bytes contained in the buffer.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.inner.len()
  }

  /// Returns `true` when the buffer is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }
}

impl Deref for Bytes {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.inner.as_slice()
  }
}

impl fmt::Debug for Bytes {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Bytes(len={})", self.len())
  }
}

impl AsRef<[u8]> for Bytes {
  fn as_ref(&self) -> &[u8] {
    self.inner.as_slice()
  }
}
