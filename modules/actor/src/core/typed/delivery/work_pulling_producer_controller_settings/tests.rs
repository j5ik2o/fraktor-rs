use crate::core::typed::delivery::WorkPullingProducerControllerSettings;

#[test]
fn default_settings() {
  let settings = WorkPullingProducerControllerSettings::new();
  assert_eq!(settings.buffer_size(), 1000);
}

#[test]
fn default_trait() {
  let settings = WorkPullingProducerControllerSettings::default();
  assert_eq!(settings.buffer_size(), 1000);
}
