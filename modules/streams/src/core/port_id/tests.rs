use crate::core::PortId;

#[test]
fn port_ids_are_unique() {
  let a = PortId::next();
  let b = PortId::next();
  assert_ne!(a, b);
}
