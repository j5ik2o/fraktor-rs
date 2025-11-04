use cellactor_actor_core_rs::{
  dispatcher::{
    DispatchExecutor as CoreDispatchExecutor, DispatchHandle as CoreDispatchHandle, Dispatcher as CoreDispatcher,
  },
  mailbox::Mailbox,
  props::DispatcherConfig as CoreDispatcherConfig,
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;

/// Dispatch handle specialised for `StdToolbox`.
pub type DispatchHandle = CoreDispatchHandle<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = CoreDispatcher<StdToolbox>;
