use alloc::string::String;

use super::super::{
  consistent_hashable_envelope::ConsistentHashableEnvelope,
  consistent_hashing_pool::{ConsistentHashingHashKeyMapperKind, ConsistentHashingPool},
  consistent_hashing_routing_logic::ConsistentHashingRoutingLogic,
  pool::Pool,
  routee::Routee,
  router_config::RouterConfig,
  routing_logic::RoutingLogic,
};
use crate::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  messaging::AnyMessage,
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

fn hash_key_from_u32(message: &AnyMessage) -> u64 {
  u64::from(*message.downcast_ref::<u32>().expect("u32 message"))
}

#[test]
fn new_creates_pool() {
  let pool = ConsistentHashingPool::new(4, hash_key_from_u32);
  assert_eq!(pool.nr_of_instances(), 4);
}

#[test]
#[should_panic(expected = "nr_of_instances must be positive")]
fn new_panics_on_zero_instances() {
  drop(ConsistentHashingPool::new(0, hash_key_from_u32));
}

#[test]
fn new_envelope_hash_key_marks_pool_as_wire_safe() {
  let pool = ConsistentHashingPool::new_envelope_hash_key(4);

  assert_eq!(pool.nr_of_instances(), 4);
  assert_eq!(pool.hash_key_mapper_kind(), ConsistentHashingHashKeyMapperKind::EnvelopeHashKey);
}

#[test]
#[should_panic(expected = "nr_of_instances must be positive")]
fn new_envelope_hash_key_panics_on_zero_instances() {
  drop(ConsistentHashingPool::new_envelope_hash_key(0));
}

#[test]
fn create_router_returns_functional_router() {
  let pool = ConsistentHashingPool::new(3, hash_key_from_u32);
  let router = pool.create_router();
  let routees = vec![make_routee(1), make_routee(2), make_routee(3)];
  let router = router.with_routees(routees);
  assert_eq!(router.routees().len(), 3);
}

#[test]
fn logic_selects_same_routee_for_same_key() {
  // ConsistentHashingRoutingLogic を直接テストする
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = [make_routee(10), make_routee(20), make_routee(30)];

  let msg1 = AnyMessage::new(7_u32);
  let msg2 = AnyMessage::new(7_u32);

  // 同じキーからは同じ routee が選択される
  let selected1 = logic.select(&msg1, &routees);
  let selected2 = logic.select(&msg2, &routees);
  assert_eq!(selected1, selected2);
}

#[test]
fn router_dispatcher_defaults_to_default() {
  let pool = ConsistentHashingPool::new(2, hash_key_from_u32);
  assert_eq!(pool.router_dispatcher(), "default-dispatcher");
}

#[test]
fn with_dispatcher_overrides_default() {
  let pool = ConsistentHashingPool::new(2, hash_key_from_u32).with_dispatcher(String::from("custom-dispatcher"));
  assert_eq!(pool.router_dispatcher(), "custom-dispatcher");
}

#[test]
fn envelope_hash_key_router_uses_envelope_key_for_selection() {
  let pool = ConsistentHashingPool::new_envelope_hash_key(3);
  let logic = pool.create_routing_logic();
  let routees = [make_routee(10), make_routee(20), make_routee(30)];
  let first_message = AnyMessage::new(ConsistentHashableEnvelope::new(AnyMessage::new(7_u32), 0_u64));
  let first_routee = logic.select(&first_message, &routees);
  let different_key = (1_u64..10_000)
    .find(|key| {
      let message = AnyMessage::new(ConsistentHashableEnvelope::new(AnyMessage::new(7_u32), *key));
      logic.select(&message, &routees) != first_routee
    })
    .expect("test routees should produce at least two selections");
  let second_message = AnyMessage::new(ConsistentHashableEnvelope::new(AnyMessage::new(7_u32), different_key));

  assert_ne!(logic.select(&first_message, &routees), logic.select(&second_message, &routees));
}

#[test]
fn has_resizer_defaults_to_false() {
  let pool = ConsistentHashingPool::new(2, hash_key_from_u32);
  assert!(!pool.has_resizer());
}
