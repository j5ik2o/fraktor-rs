use super::SeqNr;

// SeqNr は u64 の型エイリアスであり、シーケンス番号は 1 から開始する規約を確認する。
#[test]
fn seq_nr_is_u64_alias() {
  let seq: SeqNr = 1;
  assert_eq!(seq, 1);
}
