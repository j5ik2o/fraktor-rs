use super::SingletonStuckPhase;

#[test]
fn becoming_oldest_equals_itself() {
  assert_eq!(SingletonStuckPhase::BecomingOldest, SingletonStuckPhase::BecomingOldest);
}

#[test]
fn handing_over_equals_itself() {
  assert_eq!(SingletonStuckPhase::HandingOver, SingletonStuckPhase::HandingOver);
}

#[test]
fn becoming_oldest_not_equals_handing_over() {
  assert_ne!(SingletonStuckPhase::BecomingOldest, SingletonStuckPhase::HandingOver);
}
