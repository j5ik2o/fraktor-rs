use super::RestartBackoff;
use crate::RestartConfig;

#[test]
fn reset_backoff_skips_when_restart_window_is_disabled() {
  // max_restarts_within_ticks = 0 はウィンドウ無効を意味し、リセットしない
  let config = RestartConfig::new(2, 16, 3).with_max_restarts_within_ticks(0);
  let mut backoff = RestartBackoff::from_config(config);
  backoff.current_backoff_ticks = 8;

  backoff.reset_backoff_if_window_elapsed(100);

  assert_eq!(backoff.current_backoff_ticks, 8);
}

#[test]
fn reset_backoff_restores_min_backoff_after_window_elapsed() {
  // 最終スケジュールからウィンドウを超えた場合、バックオフを最小値へ戻す
  let config = RestartConfig::new(2, 16, 3).with_max_restarts_within_ticks(4);
  let mut backoff = RestartBackoff::from_config(config);
  backoff.current_backoff_ticks = 8;
  backoff.last_schedule_tick = 1;

  backoff.reset_backoff_if_window_elapsed(10);

  assert_eq!(backoff.current_backoff_ticks, 2);
}

#[test]
fn reset_backoff_keeps_current_backoff_within_window() {
  // ウィンドウ内では現在のバックオフを維持する
  let config = RestartConfig::new(2, 16, 3).with_max_restarts_within_ticks(4);
  let mut backoff = RestartBackoff::from_config(config);
  backoff.current_backoff_ticks = 8;
  backoff.last_schedule_tick = 1;

  backoff.reset_backoff_if_window_elapsed(3);

  assert_eq!(backoff.current_backoff_ticks, 8);
}
