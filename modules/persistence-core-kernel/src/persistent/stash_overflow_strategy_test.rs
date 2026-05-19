use crate::persistent::StashOverflowStrategy;

#[test]
fn stash_overflow_strategy_variants_are_distinct() {
  assert_ne!(StashOverflowStrategy::Drop, StashOverflowStrategy::Fail);
}
