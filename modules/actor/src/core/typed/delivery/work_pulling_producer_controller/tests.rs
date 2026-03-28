use alloc::string::String;

use crate::core::typed::{
  delivery::{
    ConsumerControllerCommand, WorkPullingProducerController, WorkPullingProducerControllerCommand,
    WorkPullingProducerControllerSettings,
  },
  receptionist::ServiceKey,
};

#[test]
fn work_pulling_producer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<WorkPullingProducerControllerCommand<String>>();

  let key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");
  let _behavior = WorkPullingProducerController::behavior::<u32>("test-producer", key);
}

#[test]
fn work_pulling_producer_controller_with_settings_compiles() {
  let key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");
  let settings = WorkPullingProducerControllerSettings::new();
  let _behavior = WorkPullingProducerController::behavior_with_settings::<u32>("test-producer", key, &settings);
}
