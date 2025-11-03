use super::MailboxOverflowStrategy;

#[test]
fn strategy_variants_exist() {
  assert!(matches!(MailboxOverflowStrategy::DropNewest, MailboxOverflowStrategy::DropNewest));
  assert!(matches!(MailboxOverflowStrategy::DropOldest, MailboxOverflowStrategy::DropOldest));
  assert!(matches!(MailboxOverflowStrategy::Grow, MailboxOverflowStrategy::Grow));
  assert!(matches!(MailboxOverflowStrategy::Block, MailboxOverflowStrategy::Block));
}
