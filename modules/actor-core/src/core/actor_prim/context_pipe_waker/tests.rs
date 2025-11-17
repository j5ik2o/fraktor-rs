use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::{ContextPipeTaskId, context_pipe_waker::ContextPipeWaker},
  system::SystemStateGeneric,
};

#[test]
fn context_pipe_waker_sends_system_message() {
  let state = ArcShared::new(SystemStateGeneric::<NoStdToolbox>::new());
  let pid = state.allocate_pid();
  // The actor isn't registered, but sending the system message should still be a no-op.
  let waker = ContextPipeWaker::<NoStdToolbox>::into_waker(state.clone(), pid, ContextPipeTaskId::new(1));
  waker.wake_by_ref();
}
