use crate::core::{grain_key::GrainKey, virtual_actor_event::VirtualActorEvent};

#[test]
fn activated_event_carries_fields() {
  let key = GrainKey::new("user:1".to_string());
  let ev = VirtualActorEvent::Activated { key: key.clone(), pid: "pid-1".to_string(), authority: "a1".to_string() };
  assert!(matches!(ev, VirtualActorEvent::Activated { .. }));
  if let VirtualActorEvent::Activated { key: k, pid, authority } = ev {
    assert_eq!(k, key);
    assert_eq!(pid, "pid-1");
    assert_eq!(authority, "a1");
  }
}
