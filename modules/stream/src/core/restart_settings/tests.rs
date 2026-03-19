use crate::core::RestartSettings;

#[test]
fn restart_settings_normalizes_max_backoff() {
  let settings = RestartSettings::new(5, 1, 3);
  assert_eq!(settings.min_backoff_ticks(), 5);
  assert_eq!(settings.max_backoff_ticks(), 5);
}

#[test]
fn restart_settings_clamps_random_factor_permille() {
  let settings = RestartSettings::new(1, 8, 3).with_random_factor_permille(1500);
  assert_eq!(settings.random_factor_permille(), 1000);
}
