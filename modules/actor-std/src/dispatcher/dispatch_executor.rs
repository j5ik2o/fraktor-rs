//! Dispatch executors bridging core dispatcher logic to host runtimes.

mod thread_executor;
#[cfg(feature = "tokio-executor")]
mod tokio_executor;

pub use thread_executor::ThreadedExecutor;
#[cfg(feature = "tokio-executor")]
pub use tokio_executor::TokioExecutor;
