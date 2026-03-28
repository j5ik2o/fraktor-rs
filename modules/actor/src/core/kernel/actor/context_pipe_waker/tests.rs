use crate::core::kernel::{
  actor::{ContextPipeTaskId, context_pipe_waker::ContextPipeWaker},
  system::state::{SystemStateShared, system_state::SystemState},
};

#[test]
fn context_pipe_waker_sends_system_message() {
  let state = SystemStateShared::new(SystemState::new());
  let pid = state.allocate_pid();
  // The actor isn't registered, but sending the system message should still be a no-op.
  let waker = ContextPipeWaker::into_waker(state.clone(), pid, ContextPipeTaskId::new(1));
  waker.wake_by_ref();
}
