use super::ActorLogMarker;

#[test]
fn dead_letter_marker_uses_pekko_marker_name_and_property() {
  let marker = ActorLogMarker::dead_letter("ExampleMessage");

  assert_eq!(marker.name(), "pekkoDeadLetter");
  assert_eq!(marker.properties().get("pekkoMessageClass").map(String::as_str), Some("ExampleMessage"));
}
