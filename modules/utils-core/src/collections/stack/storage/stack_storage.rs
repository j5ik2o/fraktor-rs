/// Low-level storage abstraction used by stack backends.
pub trait StackStorage<T> {
  /// Returns the capacity of the storage.
  fn capacity(&self) -> usize;

  /// Reads an element at the specified index without bounds checks.
  ///
  /// # Safety
  ///
  /// The caller must ensure the index satisfies the storage invariants.
  unsafe fn read_unchecked(&self, idx: usize) -> *const T;

  /// Writes an element at the specified index without bounds checks.
  ///
  /// # Safety
  ///
  /// The caller must ensure the index satisfies the storage invariants.
  unsafe fn write_unchecked(&mut self, idx: usize, val: T);
}
