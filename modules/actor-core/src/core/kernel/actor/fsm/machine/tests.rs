use core::hash::Hash;

use super::Fsm;

impl<State, Data> Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Sets the named timer generation counter for wrap-around tests.
  pub(crate) const fn set_named_timer_generation_for_test(&mut self, generation: u64) {
    self.named_timer_generation = generation;
  }
}
