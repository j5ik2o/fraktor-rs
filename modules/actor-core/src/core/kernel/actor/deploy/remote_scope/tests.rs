use crate::core::kernel::actor::{Address, deploy::RemoteScope};

#[test]
fn new_preserves_target_node_address() {
  let node = Address::remote("remote-sys", "10.0.0.2", 2553);

  let scope = RemoteScope::new(node.clone());

  assert_eq!(scope.node(), &node);
}
