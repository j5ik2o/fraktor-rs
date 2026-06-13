use core::{num::NonZeroUsize, time::Duration};

use crate::ddata::ReadConsistency;

#[test]
fn read_consistency_variants_keep_parameters() {
  let timeout = Duration::from_secs(5);
  let two = NonZeroUsize::new(2).expect("2 is non-zero");

  assert_eq!(ReadConsistency::Local, ReadConsistency::Local);
  assert_eq!(ReadConsistency::From { n: two, timeout }, ReadConsistency::From { n: two, timeout });
  assert_eq!(ReadConsistency::Majority { timeout, min_cap: 3 }, ReadConsistency::Majority { timeout, min_cap: 3 });
  assert_eq!(ReadConsistency::MajorityPlus { timeout, additional: two, min_cap: 3 }, ReadConsistency::MajorityPlus {
    timeout,
    additional: two,
    min_cap: 3
  });
  assert_eq!(ReadConsistency::All { timeout }, ReadConsistency::All { timeout });
}
