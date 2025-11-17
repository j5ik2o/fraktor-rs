//! Owned wrapper around [`SchedulerTickHandle`].

use alloc::boxed::Box;

use fraktor_utils_rs::core::time::SchedulerTickHandle;

/// Owns a `'static` tick handle for scheduler drivers.
pub struct SchedulerTickHandleOwned {
  handle: SchedulerTickHandle<'static>,
  scope:  *mut TickHandleScope,
}

struct TickHandleScope;

impl SchedulerTickHandleOwned {
  /// Creates a new owned handle.
  #[must_use]
  pub fn new() -> Self {
    let scope = Box::into_raw(Box::new(TickHandleScope));
    let handle = unsafe { SchedulerTickHandle::scoped(&*scope) };
    Self { handle, scope }
  }

  /// Returns a reference to the underlying handle.
  #[must_use]
  pub const fn handle(&self) -> &SchedulerTickHandle<'static> {
    &self.handle
  }
}

impl Drop for SchedulerTickHandleOwned {
  fn drop(&mut self) {
    unsafe {
      drop(Box::from_raw(self.scope));
    }
  }
}

impl Default for SchedulerTickHandleOwned {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl Send for SchedulerTickHandleOwned {}
unsafe impl Sync for SchedulerTickHandleOwned {}
