use alloc::string::String;

use super::SelectionPathElement;

#[test]
fn child_name_keeps_exact_matcher() {
  let element = SelectionPathElement::ChildName(String::from("worker"));

  assert_eq!(element, SelectionPathElement::ChildName(String::from("worker")));
}

#[test]
fn child_pattern_keeps_wildcard_matcher() {
  let element = SelectionPathElement::ChildPattern(String::from("worker-*"));

  assert_eq!(element, SelectionPathElement::ChildPattern(String::from("worker-*")));
}

#[test]
fn parent_is_distinct_from_other_variants() {
  assert_ne!(SelectionPathElement::Parent, SelectionPathElement::ChildName(String::from("worker")));
  assert_ne!(SelectionPathElement::Parent, SelectionPathElement::ChildPattern(String::from("worker-*")));
}
