use cellactor_actor_core_rs::dispatcher::{DispatchHandle as CoreDispatchHandle, Dispatcher as CoreDispatcher};
use cellactor_utils_std_rs::StdToolbox;

/// Dispatch handle specialised for `StdToolbox`.
pub type DispatchHandle = CoreDispatchHandle<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = CoreDispatcher<StdToolbox>;
