use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::ArcShared;

use super::*;
use crate::{
  actor::{
    error::SendError,
    messaging::AnyMessage,
    props::{MailboxConfigError, MailboxRequirement},
  },
  dispatch::mailbox::{
    EnqueueOutcome, Envelope, MailboxClock, MailboxOverflowStrategy, MailboxPolicy, MailboxRegistryError,
    MailboxSelection,
  },
};

fn fixed_zero_clock() -> MailboxClock {
  let closure: Box<dyn Fn() -> Duration + Send + Sync> = Box::new(|| Duration::ZERO);
  ArcShared::from_boxed(closure)
}

struct ConstantPriority;

impl MessagePriorityGenerator for ConstantPriority {
  fn priority(&self, _message: &AnyMessage) -> i32 {
    0
  }
}

fn bounded_drop_oldest_push_timeout_policy() -> MailboxPolicy {
  let capacity = NonZeroUsize::new(1).expect("capacity");
  MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropOldest, None).with_push_timeout(Some(Duration::ZERO))
}

fn assert_push_timeout_rejects_without_eviction(config: MailboxConfig, label: &str) {
  let queue = create_message_queue_from_config(&config).expect(label);
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("fill queue");

  let clock = fixed_zero_clock();
  let result = queue.enqueue_with_mailbox_clock(Envelope::new(AnyMessage::new(2_u32)), Some(&clock));
  let error = result.expect_err(label);
  assert!(matches!(error.error(), SendError::Timeout(_)), "{label}");
  assert_eq!(error.error().message().payload().downcast_ref::<u32>().copied(), Some(2_u32), "{label}");

  let retained = queue.dequeue().expect("dequeue retained").into_payload();
  assert_eq!(
    retained.payload().downcast_ref::<u32>().copied(),
    Some(1_u32),
    "{label} must retain the existing envelope instead of applying DropOldest",
  );
}

#[test]
fn register_and_resolve_mailbox() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let config = MailboxConfig::default().with_warn_threshold(None);
  registry.register("custom", config).expect("register mailbox");
  assert!(registry.resolve("custom").is_ok());
}

#[test]
fn register_duplicate_mailbox_fails() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let config = MailboxConfig::default();
  registry.register("dup", config).expect("first register");
  assert!(matches!(registry.register("dup", MailboxConfig::default()), Err(MailboxRegistryError::Duplicate(_))));
}

#[test]
fn ensure_default_mailbox_is_available() {
  let mut registry = Mailboxes::default();
  registry.ensure_default();
  assert!(registry.resolve(DEFAULT_MAILBOX_ID).is_ok());
  assert!(registry.lookup_by_queue_type(MailboxRequirement::none()).is_ok());
}

#[test]
fn lookup_by_queue_type_uses_requirement_binding() {
  let mut registry = Mailboxes::new();
  registry.register("multi", MailboxConfig::default()).expect("register mailbox");
  registry.bind_queue_type(MailboxRequirement::requires_multiple_consumer(), "multi");

  let factory = registry.lookup_by_queue_type(MailboxRequirement::requires_multiple_consumer()).expect("lookup");

  assert!(factory.create_message_queue().is_ok());
}

#[test]
fn select_uses_explicit_then_dispatcher_then_requirements() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  registry.register("explicit", MailboxConfig::default()).expect("explicit");
  registry.register("dispatcher", MailboxConfig::default()).expect("dispatcher");
  registry.register("actor-req", MailboxConfig::default()).expect("actor req");
  registry.register("dispatcher-req", MailboxConfig::default()).expect("dispatcher req");
  registry.bind_queue_type(MailboxRequirement::requires_deque(), "actor-req");
  registry.bind_queue_type(MailboxRequirement::requires_multiple_consumer(), "dispatcher-req");

  let explicit = MailboxSelection::new()
    .with_explicit_mailbox_id("explicit")
    .with_dispatcher_mailbox_id("dispatcher")
    .with_actor_requirement(MailboxRequirement::requires_deque())
    .with_dispatcher_requirement(MailboxRequirement::requires_multiple_consumer());
  let dispatcher = MailboxSelection::new()
    .with_dispatcher_mailbox_id("dispatcher")
    .with_actor_requirement(MailboxRequirement::requires_deque());
  let actor_requirement = MailboxSelection::new()
    .with_actor_requirement(MailboxRequirement::requires_deque())
    .with_dispatcher_requirement(MailboxRequirement::requires_multiple_consumer());
  let dispatcher_requirement =
    MailboxSelection::new().with_dispatcher_requirement(MailboxRequirement::requires_multiple_consumer());

  assert!(ArcShared::ptr_eq(
    &registry.select(&explicit).expect("select explicit"),
    &registry.resolve("explicit").unwrap()
  ));
  assert!(ArcShared::ptr_eq(
    &registry.select(&dispatcher).expect("select dispatcher"),
    &registry.resolve("dispatcher").unwrap()
  ));
  assert!(ArcShared::ptr_eq(
    &registry.select(&actor_requirement).expect("select actor req"),
    &registry.resolve("actor-req").unwrap()
  ));
  assert!(ArcShared::ptr_eq(
    &registry.select(&dispatcher_requirement).expect("select dispatcher req"),
    &registry.resolve("dispatcher-req").unwrap()
  ));
}

#[test]
fn create_message_queue_uses_registered_mailbox_policy() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(1).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None));
  registry.register("bounded", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded").expect("create queue");
  assert!(queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).is_ok());
  let overflow_result = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  assert!(
    matches!(overflow_result, Ok(EnqueueOutcome::Rejected(_))),
    "DropNewest overflow must surface Ok(Rejected), got {overflow_result:?}",
  );
}

#[test]
fn unbounded_policy_selects_lock_free_user_queue() {
  let queue = create_message_queue_from_policy(MailboxPolicy::unbounded(None));

  assert!(!queue.requires_put_lock_for_enqueue(), "default unbounded mailbox must use the queue-local close protocol");
  assert!(queue.as_deque().is_none(), "default unbounded queue must not acquire deque semantics");
}

#[test]
fn bounded_queue_selection_keeps_lock_backed_enqueue_path() {
  let capacity = NonZeroUsize::new(2).expect("capacity");

  let bounded =
    create_message_queue_from_policy(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None));
  assert!(bounded.requires_put_lock_for_enqueue());
}

#[test]
fn deque_queue_selection_keeps_lock_backed_enqueue_path() {
  let deque_config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque());
  let deque = create_message_queue_from_config(&deque_config).expect("deque queue");
  assert!(deque.requires_put_lock_for_enqueue());
  assert!(deque.as_deque().is_some());
}

#[test]
fn control_aware_queue_selection_keeps_lock_backed_enqueue_path() {
  let control_config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  let control_aware = create_message_queue_from_config(&control_config).expect("control-aware queue");
  assert!(control_aware.requires_put_lock_for_enqueue());
}

#[test]
fn multiple_consumer_queue_selection_keeps_lock_backed_enqueue_path() {
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_multiple_consumer());
  let queue = create_message_queue_from_config(&config).expect("multiple-consumer queue");

  assert!(queue.requires_put_lock_for_enqueue());
}

#[test]
fn priority_queue_selection_keeps_lock_backed_enqueue_path() {
  let priority_config = MailboxConfig::default().with_priority_generator(ArcShared::new(ConstantPriority));
  let priority = create_message_queue_from_config(&priority_config).expect("priority queue");
  assert!(priority.requires_put_lock_for_enqueue());
}

#[test]
fn stable_priority_queue_selection_keeps_lock_backed_enqueue_path() {
  let stable_priority_config =
    MailboxConfig::default().with_priority_generator(ArcShared::new(ConstantPriority)).with_stable_priority(true);
  let stable_priority = create_message_queue_from_config(&stable_priority_config).expect("stable priority queue");
  assert!(stable_priority.requires_put_lock_for_enqueue());
}

#[test]
fn bounded_priority_selection_without_push_timeout_uses_priority_queue() {
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_priority_generator(ArcShared::new(ConstantPriority));

  let queue = create_message_queue_from_config(&config).expect("bounded priority queue");

  assert!(queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).is_ok());
}

#[test]
fn bounded_stable_priority_selection_without_push_timeout_uses_stable_priority_queue() {
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_priority_generator(ArcShared::new(ConstantPriority))
    .with_stable_priority(true);

  let queue = create_message_queue_from_config(&config).expect("bounded stable priority queue");

  assert!(queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).is_ok());
}

#[test]
fn bounded_multiple_consumer_selection_uses_bounded_mailbox_type() {
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_multiple_consumer());

  let queue = create_message_queue_from_config(&config).expect("bounded multiple-consumer queue");

  assert!(queue.requires_put_lock_for_enqueue());
}

#[test]
fn create_message_queue_passes_push_timeout_to_bounded_selection_paths() {
  assert_push_timeout_rejects_without_eviction(
    MailboxConfig::new(bounded_drop_oldest_push_timeout_policy()),
    "default bounded mailbox",
  );
  assert_push_timeout_rejects_without_eviction(
    MailboxConfig::new(bounded_drop_oldest_push_timeout_policy())
      .with_requirement(MailboxRequirement::requires_deque()),
    "bounded deque mailbox",
  );
  assert_push_timeout_rejects_without_eviction(
    MailboxConfig::new(bounded_drop_oldest_push_timeout_policy())
      .with_requirement(MailboxRequirement::requires_control_aware()),
    "bounded control-aware mailbox",
  );
  assert_push_timeout_rejects_without_eviction(
    MailboxConfig::new(bounded_drop_oldest_push_timeout_policy())
      .with_priority_generator(ArcShared::new(ConstantPriority)),
    "bounded priority mailbox",
  );
  assert_push_timeout_rejects_without_eviction(
    MailboxConfig::new(bounded_drop_oldest_push_timeout_policy())
      .with_priority_generator(ArcShared::new(ConstantPriority))
      .with_stable_priority(true),
    "bounded stable-priority mailbox",
  );
}

#[test]
fn select_falls_back_to_dispatcher_requirement_when_actor_requirement_missing() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  registry.register("dispatcher-req", MailboxConfig::default()).expect("dispatcher req");
  registry.bind_queue_type(MailboxRequirement::requires_multiple_consumer(), "dispatcher-req");
  let selection = MailboxSelection::new()
    .with_actor_requirement(MailboxRequirement::requires_deque())
    .with_dispatcher_requirement(MailboxRequirement::requires_multiple_consumer());

  let selected = registry.select(&selection).expect("select dispatcher fallback");

  assert!(ArcShared::ptr_eq(&selected, &registry.resolve("dispatcher-req").expect("resolve dispatcher")));
}

#[test]
fn select_returns_actor_requirement_error_without_dispatcher_fallback() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let selection = MailboxSelection::new().with_actor_requirement(MailboxRequirement::requires_deque());

  let result = registry.select(&selection);

  assert!(matches!(result, Err(MailboxRegistryError::Unknown(_))));
}

#[test]
fn select_uses_default_when_no_selection_criteria_are_present() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let selection = MailboxSelection::new();

  let selected = registry.select(&selection).expect("select default");

  assert!(ArcShared::ptr_eq(&selected, &registry.resolve(DEFAULT_MAILBOX_ID).expect("resolve default")));
}

#[test]
fn create_message_queue_rejects_stable_priority_without_generator() {
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_stable_priority(true);
  registry.register("bad", config).expect("register mailbox");

  let result = registry.create_message_queue("bad");
  assert!(matches!(
    result,
    Err(MailboxRegistryError::InvalidConfig(MailboxConfigError::StablePriorityWithoutGenerator))
  ));
}

#[test]
fn create_message_queue_from_control_aware_requirement() {
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  registry.register("ctrl", config).expect("register mailbox");

  let queue = registry.create_message_queue("ctrl").expect("create queue");
  assert!(queue.enqueue(Envelope::new(AnyMessage::new(42_u32))).is_ok());
  assert!(queue.has_messages());
}

#[test]
fn create_message_queue_creates_bounded_deque_for_bounded_plus_deque() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_deque());
  registry.register("bounded-deque", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded-deque").expect("create queue");
  assert!(queue.as_deque().is_some(), "bounded + deque must expose deque capability");

  // capacity=2 の DropNewest なので 3 件目は Rejected になる。
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue B");
  let overflow = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  assert!(
    matches!(overflow, Ok(EnqueueOutcome::Rejected(_))),
    "bounded + deque DropNewest overflow must be Rejected, got {overflow:?}",
  );
}

#[test]
fn create_message_queue_creates_bounded_control_aware_for_bounded_plus_control_aware() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_control_aware());
  registry.register("bounded-control-aware", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded-control-aware").expect("create queue");

  // control_X と normal_A で capacity=2 を埋めた状態。次の normal は Rejected。
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control");
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");
  let overflow = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  assert!(
    matches!(overflow, Ok(EnqueueOutcome::Rejected(_))),
    "bounded + control_aware DropNewest overflow must be Rejected, got {overflow:?}",
  );

  // control が優先的に dequeue される (control_aware 挙動)。
  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert!(first.is_control());
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));
}
