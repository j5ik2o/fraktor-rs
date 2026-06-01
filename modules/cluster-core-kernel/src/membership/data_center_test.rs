use crate::membership::DataCenter;

#[test]
fn default_data_center_is_observable() {
  assert_eq!(DataCenter::default().as_str(), "default");
}

#[test]
fn explicit_data_center_keeps_name() {
  assert_eq!(DataCenter::new("dc-east").as_str(), "dc-east");
}
