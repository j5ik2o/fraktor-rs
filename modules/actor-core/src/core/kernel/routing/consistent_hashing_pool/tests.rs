use alloc::string::String;

use super::super::{
  consistent_hashing_pool::ConsistentHashingPool, consistent_hashing_routing_logic::ConsistentHashingRoutingLogic,
  pool::Pool, routee::Routee, router_config::RouterConfig, routing_logic::RoutingLogic,
};
use crate::core::kernel::actor::{
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
  let _ = ConsistentHashingPool::new(0, hash_key_from_u32);
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
fn has_resizer_defaults_to_false() {
  let pool = ConsistentHashingPool::new(2, hash_key_from_u32);
  assert!(!pool.has_resizer());
}
