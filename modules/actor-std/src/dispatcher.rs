//! Dispatcher bindings tailored for the standard runtime facade.

mod base;
/// Dispatch executor implementations for the standard runtime.
pub mod dispatch_executor {
  //! Dispatch executors specialised for `StdToolbox`.

  pub mod thread_executor;
  pub mod tokio_executor;

  pub use thread_executor::ThreadedExecutor;
  pub use tokio_executor::TokioExecutor;
}
/// Dispatcher configuration bindings tailored for `StdToolbox`.
mod dispatcher_config;
/// Type aliases that expose core dispatcher handles in std environments.
mod types;

pub use base::*;
pub use dispatcher_config::DispatcherConfig;
pub use types::*;
