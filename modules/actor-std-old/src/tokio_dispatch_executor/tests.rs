use tokio::runtime::Runtime;

use super::TokioDispatchExecutor;

#[test]
fn execute_uses_runtime_handle() {
  let runtime = Runtime::new().expect("failed to create runtime");
  let handle = runtime.handle().clone();
  let executor = TokioDispatchExecutor::new(handle);
  let join = executor.handle().spawn_blocking(|| 42);
  let result = runtime.block_on(join).expect("task completed");
  assert_eq!(result, 42);
}
