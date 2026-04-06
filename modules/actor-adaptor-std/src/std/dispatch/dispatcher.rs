//! Dispatcher bindings tailored for the standard runtime facade.

mod blocking_dispatcher;
#[cfg(feature = "tokio-executor")]
mod default_dispatcher;
mod dispatch_executor;
/// Pinned dispatcher that dedicates a single execution lane per actor.
mod pinned_dispatcher;
mod pinned_executor;
mod schedule_adapter;

pub use blocking_dispatcher::BlockingDispatcher;
#[cfg(feature = "tokio-executor")]
pub use default_dispatcher::DefaultDispatcher;
pub use pinned_dispatcher::PinnedDispatcher;
pub use schedule_adapter::StdScheduleAdapter;
