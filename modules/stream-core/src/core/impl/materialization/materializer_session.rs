use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex};

use super::StreamState;
use crate::core::{
  KillSwitchState, KillSwitchStateHandle, StreamError, StreamPlan,
  r#impl::{fusing::StreamBufferConfig, interpreter::GraphInterpreter},
  materialization::DriveOutcome,
};

/// Internal stream execution state.
pub(crate) struct Stream {
  interpreter:               GraphInterpreter,
  kill_switch_state:         KillSwitchStateHandle,
  linked_kill_switch_states: Vec<KillSwitchStateHandle>,
}

impl Stream {
  pub(crate) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let linked_kill_switch_states = plan.shared_kill_switch_states().to_vec();
    let kill_switch_state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running));
    Self { interpreter: GraphInterpreter::new(plan, buffer_config), kill_switch_state, linked_kill_switch_states }
  }

  pub(crate) fn start(&mut self) -> Result<(), StreamError> {
    self.interpreter.start()
  }

  pub(crate) const fn state(&self) -> StreamState {
    self.interpreter.state()
  }

  pub(crate) fn drive(&mut self) -> DriveOutcome {
    if let Some(error) = self.abort_error_from_kill_switches() {
      let was_terminal = self.interpreter.state().is_terminal();
      self.interpreter.abort(&error);
      return if was_terminal { DriveOutcome::Idle } else { DriveOutcome::Progressed };
    }

    if self.shutdown_requested_from_kill_switches()
      && let Err(error) = self.interpreter.request_shutdown()
    {
      self.interpreter.abort(&error);
      return DriveOutcome::Progressed;
    }

    self.interpreter.drive()
  }

  pub(crate) fn cancel(&mut self) -> Result<(), StreamError> {
    self.interpreter.cancel()
  }

  pub(in crate::core) fn kill_switch_state(&self) -> KillSwitchStateHandle {
    self.kill_switch_state.clone()
  }

  fn abort_error_from_kill_switches(&self) -> Option<StreamError> {
    if let KillSwitchState::Aborted(error) = self.kill_switch_state.lock().clone() {
      return Some(error);
    }

    for kill_switch_state in &self.linked_kill_switch_states {
      if let KillSwitchState::Aborted(error) = kill_switch_state.lock().clone() {
        return Some(error);
      }
    }

    None
  }

  fn shutdown_requested_from_kill_switches(&self) -> bool {
    if matches!(self.kill_switch_state.lock().clone(), KillSwitchState::Shutdown) {
      return true;
    }

    self
      .linked_kill_switch_states
      .iter()
      .any(|kill_switch_state| matches!(kill_switch_state.lock().clone(), KillSwitchState::Shutdown))
  }
}

/// Shared wrapper for [`Stream`].
pub(crate) struct StreamShared {
  inner: SharedLock<Stream>,
}

impl Clone for StreamShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl StreamShared {
  pub(crate) fn new(stream: Stream) -> Self {
    let inner = SharedLock::new_with_driver::<SpinSyncMutex<_>>(stream);
    Self { inner }
  }
}

impl SharedAccess<Stream> for StreamShared {
  fn with_read<R>(&self, f: impl FnOnce(&Stream) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Stream) -> R) -> R {
    self.inner.with_write(f)
  }
}
