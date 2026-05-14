use crate::association::Association;

impl Association {
  pub(crate) const fn set_handshake_generation_for_test(&mut self, generation: u64) {
    self.handshake_generation = generation;
  }

  pub(crate) const fn set_next_flush_id_for_test(&mut self, flush_id: u64) {
    self.next_flush_id = flush_id;
  }
}
