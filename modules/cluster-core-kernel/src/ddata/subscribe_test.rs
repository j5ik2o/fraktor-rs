use crate::ddata::{Flag, FlagKey, Subscribe, SubscribeResponse, Unsubscribe};

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}

#[test]
fn subscribe_keeps_key_and_subscriber() {
  let command = Subscribe::<Flag, u64>::new(flag_key(), 77);

  assert_eq!(command.key().id(), "flag");
  assert_eq!(command.subscriber(), &77);
}

#[test]
fn subscribe_builds_changed_event() {
  let command = Subscribe::<Flag, u64>::new(flag_key(), 77);

  let event = command.changed(Flag::disabled().switch_on());

  assert!(matches!(event, SubscribeResponse::Changed { .. }));
  assert_eq!(event.key().id(), "flag");
  assert!(event.data().expect("changed event has data").is_enabled());
}

#[test]
fn subscribe_builds_deleted_event() {
  let command = Subscribe::<Flag, u64>::new(flag_key(), 77);

  let event = command.deleted();

  assert!(matches!(event, SubscribeResponse::Deleted { .. }));
  assert_eq!(event.key().id(), "flag");
  assert!(event.data().is_none());
}

#[test]
fn unsubscribe_keeps_key_and_subscriber() {
  let command = Unsubscribe::<Flag, u64>::new(flag_key(), 77);

  assert_eq!(command.key().id(), "flag");
  assert_eq!(command.subscriber(), &77);
}
