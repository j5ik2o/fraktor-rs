//! New std-layer dispatch executors that target the redesigned `Executor` trait.
//!
//! Files inside `dispatch_new/` MUST NOT depend on the legacy `dispatch/`
//! tree (see openspec change `dispatcher-pekko-1n-redesign`). Once the
//! migration completes the legacy tree is removed in a single drop and this
//! module is renamed back to `dispatch/`.

mod pinned_executor;
mod pinned_executor_factory;
mod threaded_executor;
#[cfg(feature = "tokio-executor")]
mod tokio_executor;
#[cfg(feature = "tokio-executor")]
mod tokio_executor_factory;

pub use pinned_executor::PinnedExecutor;
pub use pinned_executor_factory::PinnedExecutorFactory;
pub use threaded_executor::ThreadedExecutor;
#[cfg(feature = "tokio-executor")]
pub use tokio_executor::TokioExecutor;
#[cfg(feature = "tokio-executor")]
pub use tokio_executor_factory::TokioExecutorFactory;
