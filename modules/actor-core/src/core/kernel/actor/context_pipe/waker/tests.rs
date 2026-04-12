use crate::core::kernel::{
  actor::context_pipe::{ContextPipeTaskId, ContextPipeWaker, ContextPipeWakerHandle},
  system::state::{SystemStateShared, system_state::SystemState},
};

#[test]
fn context_pipe_waker_sends_system_message() {
  let state = SystemStateShared::new(SystemState::new());
  let pid = state.allocate_pid();
  // The actor isn't registered, but sending the system message should still be a no-op.
  let handle = ContextPipeWakerHandle::new(state.clone(), pid, ContextPipeTaskId::new(1));
  let shared = state.context_pipe_waker_handle_shared_factory().create_context_pipe_waker_handle_shared(handle);
  let waker = ContextPipeWaker::into_waker(shared);
  waker.wake_by_ref();
}
