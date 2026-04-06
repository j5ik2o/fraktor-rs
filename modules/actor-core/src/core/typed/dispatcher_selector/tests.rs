use crate::core::typed::dispatcher_selector::DispatcherSelector;

#[test]
fn should_create_from_config() {
  let selector = DispatcherSelector::from_config("my-dispatcher");
  assert_eq!(selector, DispatcherSelector::FromConfig("my-dispatcher".into()));
}

#[test]
fn should_clone_and_compare() {
  let selector = DispatcherSelector::Blocking;
  let cloned = selector.clone();
  assert_eq!(selector, cloned);
}
