use super::SeqNr;

#[test]
fn seq_nr_starts_at_one() {
  let seq: SeqNr = 1;
  assert_eq!(seq, 1);
}
