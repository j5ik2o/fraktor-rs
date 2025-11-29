extern crate alloc;

use alloc::boxed::Box;

use fraktor_utils_rs::{
  core::sync::ArcShared,
  std::{StdSyncMutex, runtime_toolbox::StdToolbox},
};

use super::{DispatchExecutor, DispatchShared};
use crate::core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor};

/// Adapter bridging [`DispatchExecutor`] trait objects to the core runtime.
///
/// Wraps an executor in an external lock (`StdSyncMutex`) and provides
/// synchronized access when implementing `CoreDispatchExecutor`.
pub struct DispatchExecutorAdapter {
  // 外部ロックで同期を取る。呼び出し元は interior mutability を持たない
  inner: ArcShared<StdSyncMutex<Box<dyn DispatchExecutor>>>,
}

impl DispatchExecutorAdapter {
  /// Creates a new adapter wrapping the given executor with external locking.
  #[must_use]
  pub fn new(inner: ArcShared<StdSyncMutex<Box<dyn DispatchExecutor>>>) -> Self {
    Self { inner }
  }
}

impl CoreDispatchExecutor<StdToolbox> for DispatchExecutorAdapter {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    // 外部ロックを取得してから execute を呼び出す
    self.inner.lock().execute(dispatcher)
  }
}
