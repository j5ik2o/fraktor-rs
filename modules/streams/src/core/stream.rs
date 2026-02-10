use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{
  DriveOutcome, GraphInterpreter, StreamBufferConfig, StreamError, StreamPlan, StreamState,
  unique_kill_switch::{KillSwitchState, KillSwitchStateHandle},
};

/// Internal stream execution state.
pub(crate) struct Stream {
  interpreter:       GraphInterpreter,
  kill_switch_state: KillSwitchStateHandle,
}

impl Stream {
  pub(super) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let kill_switch_state =
      plan.shared_kill_switch_state().unwrap_or_else(|| ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)));
    Self { interpreter: GraphInterpreter::new(plan, buffer_config), kill_switch_state }
  }

  pub(crate) fn start(&mut self) -> Result<(), StreamError> {
    self.interpreter.start()
  }

  pub(crate) const fn state(&self) -> StreamState {
    self.interpreter.state()
  }

  pub(crate) fn drive(&mut self) -> DriveOutcome {
    match self.kill_switch_state.lock().clone() {
      | KillSwitchState::Running => {},
      | KillSwitchState::Shutdown => {
        if let Err(error) = self.interpreter.request_shutdown() {
          self.interpreter.abort(error);
          return DriveOutcome::Progressed;
        }
      },
      | KillSwitchState::Aborted(error) => {
        let was_terminal = self.interpreter.state().is_terminal();
        self.interpreter.abort(error);
        return if was_terminal { DriveOutcome::Idle } else { DriveOutcome::Progressed };
      },
    }
    self.interpreter.drive()
  }

  pub(crate) fn cancel(&mut self) -> Result<(), StreamError> {
    self.interpreter.cancel()
  }

  pub(super) fn kill_switch_state(&self) -> KillSwitchStateHandle {
    self.kill_switch_state.clone()
  }
}
