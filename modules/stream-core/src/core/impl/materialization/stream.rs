use alloc::vec::Vec;

use fraktor_actor_core_rs::system::ActorSystem;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};
use portable_atomic::{AtomicU64, Ordering};

use super::StreamState;
use crate::core::{
  KillSwitchState, KillSwitchStateHandle, KillSwitchStatus, StreamError, StreamPlan,
  r#impl::{fusing::StreamBufferConfig, interpreter::graph_interpreter::GraphInterpreter},
  materialization::DriveOutcome,
  snapshot::StreamSnapshot,
  stream_ref::StreamRefSettings,
};

static STREAM_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Internal stream execution state.
pub(crate) struct Stream {
  id: u64,
  interpreter: GraphInterpreter,
  kill_switch_state: KillSwitchStateHandle,
  linked_kill_switch_states: Vec<KillSwitchStateHandle>,
  shutdown_requested: bool,
}

enum KillSwitchDriveDecision {
  Abort(StreamError),
  Shutdown,
  Continue,
}

impl Stream {
  pub(crate) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let linked_kill_switch_states = plan.shared_kill_switch_states().to_vec();
    let kill_switch_state = Self::new_running_kill_switch_state();
    Self {
      id: STREAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
      interpreter: GraphInterpreter::new(plan, buffer_config),
      kill_switch_state,
      linked_kill_switch_states,
      shutdown_requested: false,
    }
  }

  pub(in crate::core) fn new_with_materializer_context(
    plan: StreamPlan,
    buffer_config: StreamBufferConfig,
    kill_switch_state: KillSwitchStateHandle,
    actor_system: Option<&ActorSystem>,
    stream_ref_settings: &StreamRefSettings,
  ) -> Self {
    let linked_kill_switch_states = plan.shared_kill_switch_states().to_vec();
    Self {
      id: STREAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
      interpreter: GraphInterpreter::new_with_materializer_context(
        plan,
        buffer_config,
        actor_system,
        stream_ref_settings,
      ),
      kill_switch_state,
      linked_kill_switch_states,
      shutdown_requested: false,
    }
  }

  pub(in crate::core) fn new_running_kill_switch_state() -> KillSwitchStateHandle {
    ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()))
  }

  pub(crate) const fn id(&self) -> u64 {
    self.id
  }

  pub(crate) fn start(&mut self) -> Result<(), StreamError> {
    self.interpreter.start()
  }

  pub(crate) const fn state(&self) -> StreamState {
    self.interpreter.state()
  }

  pub(crate) fn drive(&mut self) -> DriveOutcome {
    match self.kill_switch_drive_decision() {
      | KillSwitchDriveDecision::Abort(error) => {
        let was_terminal = self.interpreter.state().is_terminal();
        self.interpreter.abort(&error);
        return if was_terminal { DriveOutcome::Idle } else { DriveOutcome::Progressed };
      },
      | KillSwitchDriveDecision::Shutdown if !self.shutdown_requested => {
        if self.shutdown().is_err() {
          return DriveOutcome::Progressed;
        }
      },
      | KillSwitchDriveDecision::Shutdown | KillSwitchDriveDecision::Continue => {},
    }

    self.interpreter.drive()
  }

  pub(crate) fn shutdown(&mut self) -> Result<(), StreamError> {
    if self.shutdown_requested {
      return Ok(());
    }
    if let Err(error) = self.interpreter.request_shutdown() {
      self.shutdown_requested = true;
      self.interpreter.abort(&error);
      return Err(error);
    }
    self.shutdown_requested = true;
    Ok(())
  }

  pub(crate) fn cancel(&mut self) -> Result<(), StreamError> {
    self.interpreter.cancel()
  }

  pub(crate) fn abort(&mut self, error: &StreamError) {
    self.interpreter.abort(error);
  }

  pub(in crate::core) fn kill_switch_state(&self) -> KillSwitchStateHandle {
    self.kill_switch_state.clone()
  }

  /// Returns a diagnostic snapshot of the stream's interpreter.
  pub(in crate::core) fn snapshot(&self) -> StreamSnapshot {
    let active = self.interpreter.snapshot();
    StreamSnapshot::new(alloc::vec![active], Vec::new())
  }

  fn kill_switch_drive_decision(&self) -> KillSwitchDriveDecision {
    let mut shutdown_requested = false;
    match self.kill_switch_state.lock().status().clone() {
      | KillSwitchStatus::Aborted(error) => return KillSwitchDriveDecision::Abort(error),
      | KillSwitchStatus::Shutdown => shutdown_requested = true,
      | KillSwitchStatus::Running => {},
    }

    for kill_switch_state in &self.linked_kill_switch_states {
      match kill_switch_state.lock().status().clone() {
        | KillSwitchStatus::Aborted(error) => return KillSwitchDriveDecision::Abort(error),
        | KillSwitchStatus::Shutdown => shutdown_requested = true,
        | KillSwitchStatus::Running => {},
      }
    }

    if shutdown_requested { KillSwitchDriveDecision::Shutdown } else { KillSwitchDriveDecision::Continue }
  }
}
