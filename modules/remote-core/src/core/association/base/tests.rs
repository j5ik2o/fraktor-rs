use crate::core::association::Association;

impl Association {
  pub(crate) const fn set_handshake_generation_for_test(&mut self, generation: u64) {
    self.handshake_generation = generation;
  }
}
