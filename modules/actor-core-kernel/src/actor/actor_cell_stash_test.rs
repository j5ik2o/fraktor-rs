use super::*;

#[test]
fn unstash_messages_are_replayed_before_existing_mailbox_messages() {
  let state = ActorSystem::new_empty().state();
  let received = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let captured = received.clone();
    move || OrderedMessageActor::new(captured.clone())
  })
  .with_stash_mailbox();
  let cell =
    ActorCell::create(state.clone(), Pid::new(60, 0), None, "ordered".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  cell.new_dispatcher_shared().system_dispatch(&cell, SystemMessage::Create).expect("create");
  cell.stash_message_with_limit(AnyMessage::new(1_i32), usize::MAX).expect("stashing below limit should succeed");
  cell.mailbox().enqueue_user(AnyMessage::new(2_i32)).expect("enqueue queued");

  let unstashed = cell.unstash_messages().expect("unstash");
  assert_eq!(unstashed, 1);

  wait_until(|| received.lock().len() == 2);
  assert_eq!(received.lock().clone(), vec![1, 2]);
}

#[test]
fn stash_message_with_limit_rejects_non_deque_mailbox_without_buffering() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(61, 0), None, "stash-reject".to_string(), &props)
    .expect("create actor cell");

  let error =
    cell.stash_message_with_limit(AnyMessage::new(1_i32), usize::MAX).expect_err("non-deque stash should fail");

  assert!(ActorContext::is_stash_requires_deque_error(&error));
  assert_eq!(cell.stashed_message_len(), 0);
}

#[test]
fn unstash_message_rejects_non_deque_mailbox_without_consuming_stash() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(62, 0), None, "unstash-reject".to_string(), &props)
    .expect("create actor cell");

  cell.state.with_write(|state| state.stashed_messages.push_back(AnyMessage::new(1_i32)));

  let error = cell.unstash_message().expect_err("non-deque unstash should fail");

  assert!(ActorContext::is_stash_requires_deque_error(&error));
  assert_eq!(cell.stashed_message_len(), 1);
}

#[test]
fn unstash_messages_reject_non_deque_mailbox_without_consuming_stash() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(63, 0), None, "unstash-all-reject".to_string(), &props)
    .expect("create actor cell");

  cell.state.with_write(|state| {
    state.stashed_messages.push_back(AnyMessage::new(1_i32));
    state.stashed_messages.push_back(AnyMessage::new(2_i32));
  });

  let all_error = cell.unstash_messages().expect_err("non-deque unstash should fail");
  assert!(ActorContext::is_stash_requires_deque_error(&all_error));
  assert_eq!(cell.stashed_message_len(), 2);

  let limited_error = cell.unstash_messages_with_limit(1, Ok).expect_err("non-deque unstash with limit should fail");
  assert!(ActorContext::is_stash_requires_deque_error(&limited_error));
  assert_eq!(cell.stashed_message_len(), 2);
}

#[test]
fn empty_unstash_is_noop_even_without_deque_mailbox() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(64, 0), None, "unstash-empty".to_string(), &props)
    .expect("create actor cell");

  assert_eq!(cell.unstash_message().expect("empty unstash single"), 0);
  assert_eq!(cell.unstash_messages().expect("empty unstash all"), 0);
  assert_eq!(cell.unstash_messages_with_limit(1, Ok).expect("empty unstash limit"), 0);
}
