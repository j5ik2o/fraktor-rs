use super::MailboxConfigError;

#[test]
fn display_matches_public_contract() {
  let cases = [
    (
      MailboxConfigError::StablePriorityWithoutGenerator,
      "stable_priority requires a priority generator to be attached",
    ),
    (MailboxConfigError::PriorityWithControlAware, "priority generator and control-aware cannot be used together"),
    (MailboxConfigError::PriorityWithDeque, "priority generator and deque requirement cannot be used together"),
    (MailboxConfigError::DequeWithControlAware, "control-aware and deque requirements cannot be used together"),
  ];

  for (error, expected) in cases {
    assert_eq!(alloc::format!("{error}"), expected);
  }
}
