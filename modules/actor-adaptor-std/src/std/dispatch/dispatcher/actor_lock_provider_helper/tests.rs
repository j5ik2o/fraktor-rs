use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::ActorSystem,
};

use super::{debug_actor_lock_provider, std_actor_lock_provider};

struct SelfLoopActor {
  delivered:          Arc<AtomicUsize>,
  forwards_remaining: Arc<AtomicUsize>,
}

impl Actor for SelfLoopActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.delivered.fetch_add(1, Ordering::SeqCst);
    let should_forward =
      self.forwards_remaining.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
    if should_forward {
      let mut target = ctx.self_ref();
      target.tell(AnyMessage::new(1_u32));
    }
    Ok(())
  }
}

fn build_debug_system() -> ActorSystem {
  ActorSystem::new_empty_with(|config| config.with_lock_provider(debug_actor_lock_provider()))
}

fn build_std_helper_system() -> ActorSystem {
  ActorSystem::new_empty_with(|config| config.with_lock_provider(std_actor_lock_provider()))
}

fn build_default_system() -> ActorSystem {
  ActorSystem::new_empty()
}

fn build_self_loop_actor(system: &ActorSystem) -> (ActorRef, Arc<AtomicUsize>) {
  let state = system.state();
  let delivered = Arc::new(AtomicUsize::new(0));
  let forwards_remaining = Arc::new(AtomicUsize::new(1));
  let props = {
    let delivered = delivered.clone();
    let forwards_remaining = forwards_remaining.clone();
    Props::from_fn(move || SelfLoopActor {
      delivered:          delivered.clone(),
      forwards_remaining: forwards_remaining.clone(),
    })
  };
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "self-loop".into(), &props).expect("self-loop cell");
  state.register_cell(cell.clone());
  (cell.actor_ref(), delivered)
}

#[test]
fn debug_helper_allows_same_thread_reentrant_tell_after_sender_lock_release() {
  let system = build_debug_system();
  let (mut actor_ref, delivered) = build_self_loop_actor(&system);
  actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(delivered.load(Ordering::SeqCst), 2, "debug provider should allow the nested self tell");
}

#[test]
fn default_fallback_and_system_scoped_override_remain_independent() {
  let default_system = build_default_system();
  let (mut default_actor_ref, default_delivered) = build_self_loop_actor(&default_system);
  default_actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(default_delivered.load(Ordering::SeqCst), 2, "default provider should allow the nested self tell");

  let debug_system = build_debug_system();
  let (mut debug_actor_ref, debug_delivered) = build_self_loop_actor(&debug_system);
  debug_actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(debug_delivered.load(Ordering::SeqCst), 2, "debug override should preserve the nested self tell contract");
}

#[test]
fn std_helper_builds_a_system_and_delivers_messages() {
  let system = build_std_helper_system();
  let (mut actor_ref, delivered) = build_self_loop_actor(&system);
  actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(delivered.load(Ordering::SeqCst), 2, "std helper should build a working system");
}
