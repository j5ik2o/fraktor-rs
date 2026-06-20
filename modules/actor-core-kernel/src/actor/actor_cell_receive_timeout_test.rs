use crate::actor::{
  actor_cell::tests::*,
  actor_cell_dispatch::ActorCellInvoker,
  messaging::{message_invoker::MessageInvoker, system_message::SystemMessage},
};

#[test]
fn user_message_failure_does_not_reschedule_receive_timeout() {
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ResumeSupervisorActor);
  let parent = ActorCell::create(state.clone(), Pid::new(414, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let props = Props::from_fn(|| ReceiveTimeoutFailingActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(415, 0), Some(parent.pid()), "timeout-failure".to_string(), &props)
      .expect("create actor cell");
  state.register_cell(parent.clone());
  state.register_cell(cell.clone());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("create parent");

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let initial_handle = cell
    .receive_timeout
    .as_shared_lock()
    .with_lock(|state| state.as_ref().and_then(ReceiveTimeoutState::handle_raw))
    .expect("receive timeout handle should exist after pre_start");

  let error = invoker.invoke(AnyMessage::new(1_u32)).expect_err("user message should fail");
  assert_eq!(error, ActorError::recoverable("boom"));

  let current_handle = cell
    .receive_timeout
    .as_shared_lock()
    .with_lock(|state| state.as_ref().and_then(ReceiveTimeoutState::handle_raw))
    .expect("receive timeout handle should remain registered after failure");

  assert_eq!(current_handle, initial_handle, "failure path must not arm a fresh receive-timeout timer");
}

#[test]
fn not_influence_message_skips_reschedule() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ReceiveTimeoutNoopActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(416, 0), None, "timeout-skip".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let gen_before = current_schedule_generation(&cell);
  invoker.invoke(AnyMessage::not_influence(NonInfluencingTick)).expect("invoke");
  let gen_after = current_schedule_generation(&cell);

  assert_eq!(gen_after, gen_before, "NotInfluenceReceiveTimeout payload must skip reschedule");
}

#[test]
fn regular_message_reschedules_receive_timeout() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ReceiveTimeoutNoopActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(417, 0), None, "timeout-reset".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let gen_before = current_schedule_generation(&cell);
  invoker.invoke(AnyMessage::new(NonInfluencingTick)).expect("invoke");
  let gen_after = current_schedule_generation(&cell);

  assert_eq!(gen_after, gen_before + 1, "regular payload must cancel and reschedule (one extra schedule call)");
}
