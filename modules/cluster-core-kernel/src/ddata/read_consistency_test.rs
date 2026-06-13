use core::time::Duration;

use crate::ddata::ReadConsistency;

#[test]
fn read_consistency_variants_keep_parameters() {
  let timeout = Duration::from_secs(5);

  assert_eq!(ReadConsistency::Local, ReadConsistency::Local);
  assert_eq!(ReadConsistency::From { n: 2, timeout }, ReadConsistency::From { n: 2, timeout });
  assert_eq!(ReadConsistency::Majority { timeout, min_cap: 3 }, ReadConsistency::Majority { timeout, min_cap: 3 });
  assert_eq!(ReadConsistency::MajorityPlus { timeout, additional: 2, min_cap: 3 }, ReadConsistency::MajorityPlus {
    timeout,
    additional: 2,
    min_cap: 3
  });
  assert_eq!(ReadConsistency::All { timeout }, ReadConsistency::All { timeout });
}
