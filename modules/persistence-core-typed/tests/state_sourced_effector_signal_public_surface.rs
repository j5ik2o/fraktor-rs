#![cfg(not(target_os = "none"))]

#[path = "support/public_surface_fixture.rs"]
mod public_surface_fixture;

use fraktor_persistence_core_typed_rs::StateSourcedEffectorSignal;
use public_surface_fixture::assert_fixture_build_failure_contains;

const FORGED_SIGNAL_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::StateSourcedEffectorSignal;

fn main() {
  let _signal: StateSourcedEffectorSignal<u64> = StateSourcedEffectorSignal::StatePersisted {
    auth: Default::default(),
    state: 0,
    revision: 0,
  };
}
"#;

const REUSED_AUTH_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::StateSourcedEffectorSignal;

fn forge(signal: StateSourcedEffectorSignal<u64>) -> StateSourcedEffectorSignal<u64> {
  let auth = match signal {
    | StateSourcedEffectorSignal::RecoveryCompleted { auth, .. }
    | StateSourcedEffectorSignal::RecoveryFailed { auth, .. }
    | StateSourcedEffectorSignal::StatePersisted { auth, .. }
    | StateSourcedEffectorSignal::StateDeleted { auth, .. }
    | StateSourcedEffectorSignal::PersistenceFailed { auth, .. } => auth,
  };

  StateSourcedEffectorSignal::StatePersisted {
    auth,
    state: 999,
    revision: 999,
  }
}

fn main() {}
"#;

#[derive(Clone, Debug)]
enum PrivateMessage {
  StateSourced(Option<StateSourcedEffectorSignal<u32>>),
}

#[test]
fn state_sourced_effector_signal_can_be_wrapped_by_user_private_message() {
  let message = PrivateMessage::StateSourced(None);

  match message {
    | PrivateMessage::StateSourced(signal) => assert!(signal.is_none()),
  }
}

#[test]
fn state_sourced_effector_signal_auth_cannot_be_forged_from_external_crate() {
  assert_fixture_build_failure_contains("state-sourced-effector-signal-auth-forged", FORGED_SIGNAL_SOURCE, "E0277");
  assert_fixture_build_failure_contains("state-sourced-effector-signal-auth-reused", REUSED_AUTH_SOURCE, "E0639");
}
