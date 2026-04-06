use alloc::string::String;

use crate::core::membership::VectorClock;

#[test]
fn observe_and_merge_keep_max_counter_values() {
  let mut left = VectorClock::new();
  left.observe("node-a:2552", 2);
  left.observe("node-b:2552", 1);

  let mut right = VectorClock::new();
  right.observe("node-a:2552", 3);
  right.observe("node-c:2552", 5);

  left.merge(&right);

  assert_eq!(left.value("node-a:2552"), 3);
  assert_eq!(left.value("node-b:2552"), 1);
  assert_eq!(left.value("node-c:2552"), 5);
}

#[test]
fn has_seen_all_returns_true_only_when_every_peer_reaches_target_version() {
  let peers = vec![String::from("node-a:2552"), String::from("node-b:2552")];
  let mut clock = VectorClock::new();

  clock.observe("node-a:2552", 4);
  assert!(!clock.has_seen_all(&peers, 4));

  clock.observe("node-b:2552", 4);
  assert!(clock.has_seen_all(&peers, 4));
}
