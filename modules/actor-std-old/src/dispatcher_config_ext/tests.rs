use cellactor_actor_core_rs::DispatcherConfig;
use tokio::runtime::Runtime;

use super::TokioDispatcherConfigExt;

#[test]
fn try_tokio_current_returns_error_outside_runtime() {
  assert!(DispatcherConfig::try_tokio_current().is_err());
}

#[test]
fn try_tokio_current_succeeds_inside_runtime() {
  let runtime = Runtime::new().expect("failed to create runtime");
  runtime.block_on(async {
    let config = DispatcherConfig::try_tokio_current().expect("handle available");
    let _executor = config.executor();
  });
}

#[test]
fn from_tokio_handle_works_with_cloned_handle() {
  let runtime = Runtime::new().expect("failed to create runtime");
  let handle = runtime.handle().clone();
  let config = DispatcherConfig::from_tokio_handle(handle);
  let _executor = config.executor();
}
