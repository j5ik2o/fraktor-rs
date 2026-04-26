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
fn parent_represents_parent_step() {
  assert_eq!(SelectionPathElement::Parent, SelectionPathElement::Parent);
}
