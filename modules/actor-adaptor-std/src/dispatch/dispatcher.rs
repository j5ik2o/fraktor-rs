//! Std-layer dispatch executors targeting the `Executor` trait.

#[cfg(all(test, feature = "tokio-executor"))]
#[path = "dispatcher_test.rs"]
mod tests;

mod affinity_executor;
mod affinity_executor_factory;
mod pinned_executor;
mod pinned_executor_factory;
mod threaded_executor;
#[cfg(feature = "tokio-executor")]
mod tokio_executor;
#[cfg(feature = "tokio-executor")]
mod tokio_executor_factory;

pub use affinity_executor::AffinityExecutor;
pub use affinity_executor_factory::AffinityExecutorFactory;
pub use pinned_executor::PinnedExecutor;
pub use pinned_executor_factory::PinnedExecutorFactory;
pub use threaded_executor::ThreadedExecutor;
#[cfg(feature = "tokio-executor")]
pub use tokio_executor::TokioExecutor;
#[cfg(feature = "tokio-executor")]
pub use tokio_executor_factory::TokioExecutorFactory;
