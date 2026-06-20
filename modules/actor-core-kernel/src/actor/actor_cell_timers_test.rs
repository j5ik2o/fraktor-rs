use super::*;
use crate::actor::actor_cell::tests::*;

#[test]
fn schedule_single_timer_and_cancel_tracks_active_state() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(920, 0), None, "timer-single".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  cell
    .schedule_single_timer("tick".to_string(), AnyMessage::new(7_i32), Duration::from_millis(25))
    .expect("schedule single timer");
  assert!(cell.is_timer_active("tick"));

  cell.cancel_timer("tick");

  assert!(!cell.is_timer_active("tick"));
}

#[test]
fn cancel_all_timers_clears_fixed_delay_and_fixed_rate_entries() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(921, 0), None, "timer-periodic".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  cell
    .schedule_fixed_delay_timer(
      "delay".to_string(),
      AnyMessage::new(1_i32),
      Duration::from_millis(20),
      Duration::from_millis(20),
    )
    .expect("schedule fixed-delay timer");
  cell
    .schedule_fixed_rate_timer(
      "rate".to_string(),
      AnyMessage::new(2_i32),
      Duration::from_millis(20),
      Duration::from_millis(20),
    )
    .expect("schedule fixed-rate timer");

  assert!(cell.is_timer_active("delay"));
  assert!(cell.is_timer_active("rate"));

  cell.cancel_all_timers();

  assert!(!cell.is_timer_active("delay"));
  assert!(!cell.is_timer_active("rate"));
}
