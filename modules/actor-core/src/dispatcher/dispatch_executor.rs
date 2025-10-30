use super::dispatch_handle::DispatchHandle;

/// スケジューラがディスパッチャ実行をフックするための抽象化。
pub trait DispatchExecutor: Send + Sync {
  /// ディスパッチャの実行をスケジューラへ委譲する。
  fn execute(&self, dispatcher: DispatchHandle);
}
