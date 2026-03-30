use super::OverflowStrategy;

#[test]
fn should_copy_strategy_variants() {
  let strategy = OverflowStrategy::DropBuffer;
  let copied = strategy;
  assert_eq!(copied, OverflowStrategy::DropBuffer);
}

#[test]
fn should_compare_strategy_variants() {
  assert_ne!(OverflowStrategy::DropHead, OverflowStrategy::DropTail);
  assert_eq!(OverflowStrategy::Fail, OverflowStrategy::Fail);
}
