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
pub use dispatcher_struct::Dispatcher;
#[allow(unused_imports)]
pub use dispatcher_sender::DispatcherSender;

#[cfg(test)]
mod tests;
