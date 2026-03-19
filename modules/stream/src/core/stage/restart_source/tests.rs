use super::RestartSource;
use crate::core::RestartSettings;

#[test]
fn restart_source_with_backoff_keeps_data_path_behavior() {
  let source = RestartSource::with_backoff(crate::core::stage::Source::single(1_u32), 1, 3);
  let values = source.collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn restart_source_with_settings_keeps_data_path_behavior() {
  let settings = RestartSettings::new(1, 2, 3);
  let source = RestartSource::with_settings(crate::core::stage::Source::from_array([1_u32, 2]), settings);
  let values = source.collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}
