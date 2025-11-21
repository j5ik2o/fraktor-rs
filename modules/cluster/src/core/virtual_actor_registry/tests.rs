use crate::core::{
  activation_error::ActivationError,
  grain_key::GrainKey,
  virtual_actor_event::VirtualActorEvent,
  virtual_actor_registry::VirtualActorRegistry,
};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

#[test]
fn same_key_returns_same_pid_until_owner_changes() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  let authorities = vec!["a1:4000".to_string(), "a2:4001".to_string()];
  let k = key("user:1");

  let pid1 = registry
    .ensure_activation(k.clone(), &authorities, 1, false, None)
    .expect("activation");
  let pid2 = registry
    .ensure_activation(k.clone(), &authorities, 2, false, None)
    .expect("activation");

  assert_eq!(pid1, pid2);

  let owner = registry
    .drain_events()
    .into_iter()
    .find_map(|e| match e {
      | VirtualActorEvent::Activated { authority, .. } => Some(authority),
      | _ => None,
    })
    .expect("activated event present");

  // Hitイベン ト確認のため再度呼び出し。
  registry
    .ensure_activation(k.clone(), &authorities, 2, false, None)
    .expect("activation");

  registry.invalidate_authority(&owner);

  let _pid3 = registry
    .ensure_activation(k.clone(), &vec!["a2:4001".to_string()], 3, true, Some(vec![9]))
    .expect("reactivation");

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Activated { .. })));
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Hit { .. })));
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
}

#[test]
fn passivates_when_idle_timeout_exceeded() {
  let mut registry = VirtualActorRegistry::new(4, 60);
  let k = key("user:2");
  registry
    .ensure_activation(k.clone(), &vec!["a1:4000".to_string()], 0, false, None)
    .expect("activation");

  registry.passivate_idle(15, 10);

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
  assert!(registry.cached_pid(&k, 16).is_none());
}

#[test]
fn snapshot_missing_is_reported() {
  let mut registry = VirtualActorRegistry::new(2, 60);
  let k = key("user:3");
  let err = registry
    .ensure_activation(k.clone(), &vec!["a1:4000".to_string()], 0, true, None)
    .expect_err("should fail");
  assert_eq!(err, ActivationError::SnapshotMissing { key: "user:3".to_string() });

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::SnapshotMissing { .. })));
}
