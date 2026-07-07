use alloc::string::String;

use crate::ddata::{
  Delete, Flag, FlagKey, Get, GetResponse, ReadConsistency, ReplicatorCore, ReplicatorResponse, ReplicatorSettings,
  Subscribe, SubscribeResponse, Unsubscribe, Update, UpdateResponse, WriteConsistency,
};

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}

#[test]
fn get_local_reads_present_entry() {
  let core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let mut core = core;
  let _ =
    core.handle_update(&Update::<Flag>::new(flag_key(), WriteConsistency::Local), |_| Ok(Flag::disabled().switch_on()));

  let outcome = core.handle_get(&Get::<Flag>::new(flag_key(), ReadConsistency::Local));
  let response = match outcome.response.expect("get produces response") {
    | ReplicatorResponse::Get(response) => response,
    | _ => panic!("expected get response"),
  };

  assert!(matches!(response, GetResponse::Success { .. }));
  assert!(response.data().expect("success has data").is_enabled());
}

#[test]
fn get_non_local_returns_failure_without_mutation() {
  use core::{num::NonZeroUsize, time::Duration};

  let core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let command = Get::<Flag>::new(flag_key(), ReadConsistency::From {
    n:       NonZeroUsize::new(2).expect("non-zero"),
    timeout: Duration::from_secs(1),
  });
  let outcome = core.handle_get(&command);
  let response = match outcome.response.expect("get produces response") {
    | ReplicatorResponse::Get(response) => response,
    | _ => panic!("expected get response"),
  };
  assert!(matches!(response, GetResponse::Failure { .. }));
}

#[test]
fn update_notifies_subscribers_on_change() {
  let mut core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let subscriber = 7_u64;
  let _ = core.handle_subscribe(&Subscribe::new(flag_key(), subscriber));

  let outcome =
    core.handle_update(&Update::<Flag>::new(flag_key(), WriteConsistency::Local), |_| Ok(Flag::disabled().switch_on()));

  assert_eq!(outcome.notifications.len(), 1);
  assert_eq!(outcome.notifications[0].0, subscriber);
  assert!(matches!(outcome.notifications[0].1, SubscribeResponse::Changed { .. }));
}

#[test]
fn delete_notifies_subscribers_with_deleted_event() {
  let mut core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let subscriber = 9_u64;
  let _ = core.handle_subscribe(&Subscribe::new(flag_key(), subscriber));
  let _ =
    core.handle_update(&Update::<Flag>::new(flag_key(), WriteConsistency::Local), |_| Ok(Flag::disabled().switch_on()));

  let outcome = core.handle_delete(&Delete::<Flag>::new(flag_key(), WriteConsistency::Local));
  let response = match outcome.response.expect("delete produces response") {
    | ReplicatorResponse::Delete(response) => response,
    | _ => panic!("expected delete response"),
  };
  assert!(response.is_locally_deleted());
  assert_eq!(outcome.notifications.len(), 1);
  assert!(matches!(outcome.notifications[0].1, SubscribeResponse::Deleted { .. }));
}

#[test]
fn unsubscribe_removes_subscriber() {
  let mut core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let subscriber = 3_u64;
  let _ = core.handle_subscribe(&Subscribe::new(flag_key(), subscriber));
  let _ = core.handle_unsubscribe(&Unsubscribe::new(flag_key(), subscriber));

  let outcome =
    core.handle_update(&Update::<Flag>::new(flag_key(), WriteConsistency::Local), |_| Ok(Flag::disabled().switch_on()));
  assert!(outcome.notifications.is_empty());
}

#[test]
fn update_modify_failure_does_not_notify_subscribers() {
  let mut core = ReplicatorCore::<Flag, u64>::new(ReplicatorSettings::new());
  let _ = core.handle_subscribe(&Subscribe::new(flag_key(), 1_u64));

  let outcome =
    core.handle_update(&Update::<Flag>::new(flag_key(), WriteConsistency::Local), |_| Err(String::from("rejected")));
  let response = match outcome.response.expect("update produces response") {
    | ReplicatorResponse::Update(response) => response,
    | _ => panic!("expected update response"),
  };
  assert!(matches!(response, UpdateResponse::ModifyFailure { .. }));
  assert!(outcome.notifications.is_empty());
}
