use crate::stage::TimerGraphStageLogic;

#[test]
fn timer_graph_stage_logic_should_fire_scheduled_timer_after_delay() {
  let mut timer = TimerGraphStageLogic::new();
  timer.schedule_once(1_u64, 2_u64);

  assert!(timer.advance().is_empty());
  assert_eq!(timer.advance(), vec![1_u64]);
}

#[test]
fn timer_graph_stage_logic_should_cancel_timer() {
  let mut timer = TimerGraphStageLogic::new();
  timer.schedule_once(5_u64, 1_u64);

  assert!(timer.cancel(5_u64));
  assert!(!timer.is_timer_active(5_u64));
  assert!(timer.advance().is_empty());
}

#[test]
fn timer_graph_stage_logic_should_replace_same_key_schedule() {
  let mut timer = TimerGraphStageLogic::new();
  timer.schedule_once(9_u64, 3_u64);
  timer.schedule_once(9_u64, 1_u64);

  assert_eq!(timer.advance(), vec![9_u64]);
}
