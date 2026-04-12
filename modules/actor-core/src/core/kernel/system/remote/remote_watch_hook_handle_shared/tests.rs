use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::ActorRefProvider,
    error::ActorError,
  },
  system::{
    TerminationSignal,
    remote::{RemoteWatchHook, RemoteWatchHookHandleSharedFactory},
    shared_factory::BuiltinSpinSharedFactory,
    state::{SystemStateShared, system_state::SystemState},
  },
};

#[derive(Default)]
struct RemoteWatchHookCalls {
  watch_calls:   usize,
  unwatch_calls: usize,
}

struct RecordingRemoteWatchHookProvider {
  calls:  ArcShared<SpinSyncMutex<RemoteWatchHookCalls>>,
  signal: TerminationSignal,
}

impl RecordingRemoteWatchHookProvider {
  fn new(calls: ArcShared<SpinSyncMutex<RemoteWatchHookCalls>>) -> Self {
    let state = SystemStateShared::new(SystemState::new());
    Self { calls, signal: state.termination_signal() }
  }
}

impl ActorRefProvider for RecordingRemoteWatchHookProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Err(ActorError::fatal("not needed for this test"))
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.signal.clone()
  }
}

impl RemoteWatchHook for RecordingRemoteWatchHookProvider {
  fn handle_watch(&mut self, _target: Pid, _watcher: Pid) -> bool {
    let mut calls = self.calls.lock();
    calls.watch_calls += 1;
    true
  }

  fn handle_unwatch(&mut self, _target: Pid, _watcher: Pid) -> bool {
    let mut calls = self.calls.lock();
    calls.unwatch_calls += 1;
    true
  }
}

#[test]
fn builtin_spin_shared_factory_creates_remote_watch_hook_shared() {
  let calls = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  let shared = BuiltinSpinSharedFactory::new()
    .create_remote_watch_hook_handle_shared(RecordingRemoteWatchHookProvider::new(calls.clone()));

  assert_eq!(shared.supported_schemes(), &[ActorPathScheme::FraktorTcp]);

  let mut hook = shared.clone();
  assert!(hook.handle_watch(Pid::new(1, 0), Pid::new(2, 0)));
  assert!(hook.handle_unwatch(Pid::new(1, 0), Pid::new(2, 0)));

  let calls = calls.lock();
  assert_eq!(calls.watch_calls, 1);
  assert_eq!(calls.unwatch_calls, 1);
}
