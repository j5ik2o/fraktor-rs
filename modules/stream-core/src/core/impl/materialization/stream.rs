use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::system::ActorSystem;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};
use portable_atomic::{AtomicU64, Ordering};

use super::StreamState;
use crate::core::{
  KillSwitchState, KillSwitchStateHandle, StreamError, StreamPlan,
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
}

impl Stream {
  pub(crate) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let linked_kill_switch_states = plan.shared_kill_switch_states().to_vec();
    let kill_switch_state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running));
    Self {
      id: STREAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
      interpreter: GraphInterpreter::new(plan, buffer_config),
      kill_switch_state,
      linked_kill_switch_states,
    }
  }

  pub(crate) fn new_with_materializer_context(
    plan: StreamPlan,
    buffer_config: StreamBufferConfig,
    actor_system: Option<&ActorSystem>,
    stream_ref_settings: &StreamRefSettings,
  ) -> Self {
    let linked_kill_switch_states = plan.shared_kill_switch_states().to_vec();
    let kill_switch_state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running));
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
    }
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

  /// Returns a diagnostic snapshot of the stream's interpreter.
  pub(in crate::core) fn snapshot(&self) -> StreamSnapshot {
    let active = self.interpreter.snapshot();
    StreamSnapshot::new(alloc::vec![active], Vec::new())
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
