use crate::std::dispatch::dispatcher::DispatcherConfig;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_auto_uses_blocking_capable_executor() {
  let config = DispatcherConfig::tokio_auto();
  assert!(config.executor().supports_blocking());
}
