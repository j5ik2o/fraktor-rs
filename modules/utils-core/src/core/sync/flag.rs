#[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
use alloc::rc::Rc;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
use alloc::sync::Arc;
#[cfg(any(not(feature = "alloc"), all(feature = "alloc", not(target_has_atomic = "ptr"))))]
use core::cell::Cell;
use core::fmt::{self, Debug, Formatter};
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
mod tests;

/// Structure providing a thread-safe boolean flag
///
/// `Flag` provides a boolean flag that can be safely used in multi-threaded environments.
///
/// # Implementation Details
///
/// - When `alloc` feature is enabled: Provides thread-safe implementation using `Arc<AtomicBool>`
/// - When `alloc` feature is disabled: Provides lightweight implementation for single-threaded
///   environments using `Cell<bool>`
///
/// # Examples
///
/// ```
/// use fraktor_utils_core_rs::Flag;
///
/// let flag = Flag::new(false);
/// assert!(!flag.get());
///
/// flag.set(true);
/// assert!(flag.get());
///
/// flag.clear();
/// assert!(!flag.get());
/// ```
#[derive(Clone)]
pub struct Flag {
  #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
  inner: Arc<AtomicBool>,
  #[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
  inner: Rc<Cell<bool>>,
  #[cfg(not(feature = "alloc"))]
  inner: Cell<bool>,
}

impl Flag {
  /// Creates a new `Flag` with the specified initial value
  ///
  /// # Arguments
  ///
  /// * `value` - Initial value of the flag
  ///
  /// # Examples
  ///
  /// ```
  /// use fraktor_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// assert!(flag.get());
  /// ```
  #[must_use]
  pub fn new(value: bool) -> Self {
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    {
      Self { inner: Arc::new(AtomicBool::new(value)) }
    }

    #[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
    {
      Self { inner: Rc::new(Cell::new(value)) }
    }

    #[cfg(not(feature = "alloc"))]
    {
      Self { inner: Cell::new(value) }
    }
  }

  /// Sets the value of the flag
  ///
  /// # Arguments
  ///
  /// * `value` - New value to set
  ///
  /// # Examples
  ///
  /// ```
  /// use fraktor_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(false);
  /// flag.set(true);
  /// assert!(flag.get());
  /// ```
  pub fn set(&self, value: bool) {
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    {
      self.inner.store(value, Ordering::SeqCst);
    }

    #[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
    {
      self.inner.set(value);
    }

    #[cfg(not(feature = "alloc"))]
    {
      self.inner.set(value);
    }
  }

  /// Gets the current value of the flag
  ///
  /// # Returns
  ///
  /// Current value of the flag
  ///
  /// # Examples
  ///
  /// ```
  /// use fraktor_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// assert!(flag.get());
  /// ```
  #[must_use]
  pub fn get(&self) -> bool {
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    {
      self.inner.load(Ordering::SeqCst)
    }

    #[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
    {
      return self.inner.get();
    }

    #[cfg(not(feature = "alloc"))]
    {
      return self.inner.get();
    }
  }

  /// Clears the flag (sets it to `false`)
  ///
  /// This method is equivalent to `set(false)`.
  ///
  /// # Examples
  ///
  /// ```
  /// use fraktor_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// flag.clear();
  /// assert!(!flag.get());
  /// ```
  pub fn clear(&self) {
    self.set(false);
  }
}

impl Default for Flag {
  fn default() -> Self {
    Self::new(false)
  }
}

impl Debug for Flag {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("Flag").field("value", &self.get()).finish()
  }
}
