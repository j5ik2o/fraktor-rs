#![cfg(not(target_os = "none"))]

#[path = "support/public_surface_fixture.rs"]
mod public_surface_fixture;

use public_surface_fixture::assert_fixture_build_failure_contains;

const FORGED_SIGNAL_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::EventSourcedEffectorSignal;

fn main() {
  let _signal: EventSourcedEffectorSignal<u64, u64> = EventSourcedEffectorSignal::RecoveryCompleted {
    auth: Default::default(),
    state: 0,
    sequence_nr: 0,
  };
}
"#;

const REUSED_AUTH_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::EventSourcedEffectorSignal;

fn forge(signal: EventSourcedEffectorSignal<u64, u64>) -> EventSourcedEffectorSignal<u64, u64> {
  let auth = match signal {
    | EventSourcedEffectorSignal::RecoveryCompleted { auth, .. }
    | EventSourcedEffectorSignal::PersistedEvents { auth, .. }
    | EventSourcedEffectorSignal::PersistedSnapshot { auth, .. }
    | EventSourcedEffectorSignal::DeletedSnapshots { auth, .. }
    | EventSourcedEffectorSignal::EventSourced { auth, .. } => auth,
  };

  EventSourcedEffectorSignal::RecoveryCompleted {
    auth,
    state: 999,
    sequence_nr: 999,
  }
}

fn main() {}
"#;

#[test]
fn event_sourced_effector_signal_auth_cannot_be_forged_from_external_crate() {
  assert_fixture_build_failure_contains("event-sourced-effector-signal-auth-forged", FORGED_SIGNAL_SOURCE, "Default");
  assert_fixture_build_failure_contains(
    "event-sourced-effector-signal-auth-reused",
    REUSED_AUTH_SOURCE,
    "non-exhaustive",
  );
}
