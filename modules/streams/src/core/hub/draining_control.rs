use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::ArcShared;

type DrainFn = Box<dyn Fn() + Send + Sync>;
type IsDrainingFn = Box<dyn Fn() -> bool + Send + Sync>;

/// Control handle for initiating hub draining.
pub struct DrainingControl {
  drain_fn:       ArcShared<DrainFn>,
  is_draining_fn: ArcShared<IsDrainingFn>,
}

impl Clone for DrainingControl {
  fn clone(&self) -> Self {
    Self { drain_fn: self.drain_fn.clone(), is_draining_fn: self.is_draining_fn.clone() }
  }
}

impl DrainingControl {
  pub(in crate::core::hub) fn new_with_callback<F, G>(drain_fn: F, is_draining_fn: G) -> Self
  where
    F: Fn() + Send + Sync + 'static,
    G: Fn() -> bool + Send + Sync + 'static, {
    Self {
      drain_fn:       ArcShared::new(Box::new(drain_fn)),
      is_draining_fn: ArcShared::new(Box::new(is_draining_fn)),
    }
  }

  /// Starts draining mode.
  pub fn drain(&self) {
    (self.drain_fn)();
  }

  /// Returns true when draining mode is active.
  #[must_use]
  pub fn is_draining(&self) -> bool {
    (self.is_draining_fn)()
  }
}
