use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::ddata::SelfUniqueAddress;

#[test]
fn from_authority_parses_host_and_port() {
  let self_address = SelfUniqueAddress::from_authority("node1:8080");
  assert_eq!(self_address.unique_address().address().host(), "node1");
  assert_eq!(self_address.unique_address().address().port(), 8080);
}

#[test]
fn retains_unique_address() {
  let address = UniqueAddress::new(Address::new("sys", "node-a", 2552), 7);

  let self_address = SelfUniqueAddress::new(address.clone());

  assert_eq!(self_address.unique_address(), &address);
}
