use alloc::string::String;

use super::super::{
  pool::Pool, routee::Routee, router_config::RouterConfig, smallest_mailbox_pool::SmallestMailboxPool,
};
use crate::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

#[test]
fn new_creates_pool() {
  let pool = SmallestMailboxPool::new(4);
  assert_eq!(pool.nr_of_instances(), 4);
}

#[test]
#[should_panic(expected = "nr_of_instances must be positive")]
fn new_panics_on_zero_instances() {
  let _ = SmallestMailboxPool::new(0);
}

#[test]
fn create_router_returns_functional_router() {
  let pool = SmallestMailboxPool::new(3);
  let router = pool.create_router();
  let routees = vec![make_routee(1), make_routee(2), make_routee(3)];
  let router = router.with_routees(routees);
  assert_eq!(router.routees().len(), 3);
}

#[test]
fn router_dispatcher_defaults_to_default() {
  let pool = SmallestMailboxPool::new(2);
  assert_eq!(pool.router_dispatcher(), "default-dispatcher");
}

#[test]
fn with_dispatcher_overrides_default() {
  let pool = SmallestMailboxPool::new(2).with_dispatcher(String::from("custom-dispatcher"));
  assert_eq!(pool.router_dispatcher(), "custom-dispatcher");
}

#[test]
fn has_resizer_defaults_to_false() {
  let pool = SmallestMailboxPool::new(2);
  assert!(!pool.has_resizer());
}
