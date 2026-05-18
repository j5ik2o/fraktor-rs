use crate::extension::Remote;

impl Remote {
  pub(crate) const fn association_count_for_test(&self) -> usize {
    self.associations.len()
  }
}
