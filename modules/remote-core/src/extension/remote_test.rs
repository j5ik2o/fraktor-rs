use super::accept_inbound_handshake_request;
use crate::{
  address::{Address, UniqueAddress},
  association::{Association, AssociationState},
  extension::Remote,
  instrument::NoopInstrument,
  wire::HandshakeReq,
};

impl Remote {
  pub(crate) const fn association_count_for_test(&self) -> usize {
    self.associations.len()
  }

  pub(crate) fn association_state_for_test(&self, remote: &Address) -> Option<&AssociationState> {
    self.associations.iter().find(|association| association.remote() == remote).map(|association| association.state())
  }
}

#[test]
fn accept_inbound_handshake_request_defaults_when_association_rejects() {
  let local = UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1);
  let remote = Address::new("remote-sys", "10.0.0.1", 2552);
  let mut association = Association::new(local.clone(), remote.clone());
  let request = HandshakeReq::new(UniqueAddress::new(remote, 2), local.address().clone());

  let effects = accept_inbound_handshake_request(&mut association, &request, 200, &mut NoopInstrument, 0);

  assert!(effects.is_empty());
  assert!(matches!(association.state(), AssociationState::Idle));
}
