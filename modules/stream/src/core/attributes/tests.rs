use super::Attributes;

#[test]
fn named_creates_single_name_attribute() {
  let attributes = Attributes::named("stage-a");
  assert_eq!(attributes.names(), &[alloc::string::String::from("stage-a")]);
}

#[test]
fn and_appends_names() {
  let attributes = Attributes::named("left").and(Attributes::named("right"));
  assert_eq!(attributes.names(), &[alloc::string::String::from("left"), alloc::string::String::from("right")]);
}

#[test]
fn new_is_empty() {
  let attributes = Attributes::new();
  assert!(attributes.is_empty());
}
