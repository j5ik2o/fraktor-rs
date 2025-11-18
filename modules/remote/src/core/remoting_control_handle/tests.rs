use fraktor_actor_rs::core::actor_prim::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::sync_mutex_like::SyncMutexLike};

use super::{BackpressureSignal, RemotingControlHandle, RemotingFlightRecorder};

#[allow(dead_code)]
impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  pub(crate) fn supervisor_ref(&self) -> Option<ActorRefGeneric<TB>> {
    self.shared.supervisor.lock().clone()
  }

  pub(crate) fn test_notify_backpressure(&self, signal: BackpressureSignal, authority: &str) {
    self.notify_backpressure_internal(signal, authority);
  }

  pub(crate) fn flight_recorder_for_test(&self) -> RemotingFlightRecorder {
    self.shared.flight_recorder.clone()
  }
}
