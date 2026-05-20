use crate::{address::Address, association::AssociationState, extension::Remote};

impl Remote {
  pub(crate) const fn association_count_for_test(&self) -> usize {
    self.associations.len()
  }

  pub(crate) fn association_state_for_test(&self, remote: &Address) -> Option<&AssociationState> {
    self.associations.iter().find(|association| association.remote() == remote).map(|association| association.state())
  }
}
