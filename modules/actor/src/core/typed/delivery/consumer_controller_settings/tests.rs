use super::ConsumerControllerSettings;

#[test]
fn default_settings() {
  let settings = ConsumerControllerSettings::new();
  assert_eq!(settings.flow_control_window(), 50);
  assert!(!settings.only_flow_control());
}

#[test]
fn with_flow_control_window() {
  let settings = ConsumerControllerSettings::new().with_flow_control_window(100);
  assert_eq!(settings.flow_control_window(), 100);
}

#[test]
fn with_only_flow_control() {
  let settings = ConsumerControllerSettings::new().with_only_flow_control(true);
  assert!(settings.only_flow_control());
}
