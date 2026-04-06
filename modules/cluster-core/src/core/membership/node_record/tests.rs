use alloc::{string::String, vec};

use super::NodeRecord;
use crate::core::membership::{MembershipVersion, NodeStatus};

#[test]
fn node_record_new_keeps_app_version_and_roles() {
  let roles = vec![String::from("frontend"), String::from("edge")];
  let record = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:4050"),
    NodeStatus::Up,
    MembershipVersion::new(10),
    String::from("1.2.3"),
    roles.clone(),
  );

  assert_eq!(record.app_version, "1.2.3");
  assert_eq!(record.roles, roles);
  assert_eq!(record.join_version, MembershipVersion::new(10));
}

#[test]
fn is_older_than_uses_join_version_after_status_update() {
  let mut older = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:4050"),
    NodeStatus::Up,
    MembershipVersion::new(4),
    String::from("1.0.0"),
    vec![String::from("core")],
  );
  older.version = MembershipVersion::new(40);

  let newer = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:4050"),
    NodeStatus::Up,
    MembershipVersion::new(5),
    String::from("1.0.1"),
    vec![String::from("core")],
  );

  assert!(older.is_older_than(&newer));
  assert!(!newer.is_older_than(&older));
}

#[test]
fn is_older_than_uses_numeric_port_for_tie_breaker() {
  let lower_port = NodeRecord::new(
    String::from("node-1"),
    String::from("n1:999"),
    NodeStatus::Up,
    MembershipVersion::new(10),
    String::from("1.0.0"),
    vec![String::from("core")],
  );

  let higher_port = NodeRecord::new(
    String::from("node-2"),
    String::from("n1:10000"),
    NodeStatus::Up,
    MembershipVersion::new(10),
    String::from("1.0.0"),
    vec![String::from("core")],
  );

  assert!(lower_port.is_older_than(&higher_port));
  assert!(!higher_port.is_older_than(&lower_port));
}
