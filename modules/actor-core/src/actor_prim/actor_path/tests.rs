use alloc::string::ToString;

use super::ActorPath;

#[test]
fn actor_path_root() {
  let root = ActorPath::root();
  assert_eq!(root.to_string(), "/");
  assert!(root.segments().is_empty());
}

#[test]
fn actor_path_child_segments() {
  let path = ActorPath::root().child("user").child("guardian");
  assert_eq!(path.to_string(), "/user/guardian");
  assert_eq!(path.segments(), &["user".to_string(), "guardian".to_string()]);
}
