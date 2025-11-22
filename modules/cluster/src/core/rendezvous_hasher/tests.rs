use crate::core::{grain_key::GrainKey, rendezvous_hasher::RendezvousHasher};

#[test]
fn selects_highest_score_consistently() {
  let authorities = vec!["n1:4050".to_string(), "n2:4051".to_string()];
  let key = GrainKey::new("user:42".to_string());

  let selected = RendezvousHasher::select(&authorities, &key).expect("some authority");

  // 再計算しても同じ結果になること。
  let selected_again = RendezvousHasher::select(&authorities, &key).expect("some authority");
  assert_eq!(selected, selected_again);
}

#[test]
fn returns_none_when_no_candidates() {
  let key = GrainKey::new("user:1".to_string());
  assert!(RendezvousHasher::select(&[], &key).is_none());
}
