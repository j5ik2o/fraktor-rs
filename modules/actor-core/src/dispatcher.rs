//! Dispatcher module providing scheduling primitives.

mod dispatch_executor;
mod dispatch_handle;
mod dispatcher_core;
mod dispatcher_sender;
mod dispatcher_state;
mod dispatcher_struct;
mod inline_executor;
mod schedule_waker;

#[allow(unused_imports)]
pub use dispatch_executor::DispatchExecutor;
#[allow(unused_imports)]
pub use dispatch_handle::DispatchHandle;
#[allow(unused_imports)]
pub use dispatcher_sender::DispatcherSender;
pub use dispatcher_struct::Dispatcher;
pub use inline_executor::InlineExecutor;

#[cfg(test)]
mod tests;
