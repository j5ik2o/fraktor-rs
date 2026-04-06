//! Dispatcher bindings tailored for the standard runtime facade.

mod blocking_dispatcher_provider;
#[cfg(feature = "tokio-executor")]
mod default_dispatcher_provider;
mod dispatch_executor;
/// Pinned dispatcher that dedicates a single execution lane per actor.
mod pinned_dispatcher_provider;
mod pinned_executor;
mod schedule_adapter;

pub use blocking_dispatcher_provider::BlockingDispatcherProvider;
#[cfg(feature = "tokio-executor")]
pub use default_dispatcher_provider::DefaultDispatcherProvider;
pub use pinned_dispatcher_provider::PinnedDispatcherProvider;
pub use schedule_adapter::StdScheduleAdapter;
