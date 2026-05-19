use super::SupervisionStrategy;

#[test]
fn supervision_strategy_variants_are_distinct() {
  assert_ne!(SupervisionStrategy::Stop, SupervisionStrategy::Resume);
  assert_ne!(SupervisionStrategy::Resume, SupervisionStrategy::Restart);
}

#[test]
fn supervision_strategy_is_copy() {
  let strategy = SupervisionStrategy::Restart;
  let copied = strategy;
  assert_eq!(strategy, copied);
}
