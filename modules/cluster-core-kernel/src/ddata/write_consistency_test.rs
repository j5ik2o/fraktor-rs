use core::{num::NonZeroUsize, time::Duration};

use crate::ddata::WriteConsistency;

#[test]
fn write_consistency_variants_keep_parameters() {
  let timeout = Duration::from_secs(5);
  let two = NonZeroUsize::new(2).expect("2 is non-zero");

  assert_eq!(WriteConsistency::Local, WriteConsistency::Local);
  assert_eq!(WriteConsistency::To { n: two, timeout }, WriteConsistency::To { n: two, timeout });
  assert_eq!(WriteConsistency::Majority { timeout, min_cap: 3 }, WriteConsistency::Majority { timeout, min_cap: 3 });
  assert_eq!(WriteConsistency::MajorityPlus { timeout, additional: two, min_cap: 3 }, WriteConsistency::MajorityPlus {
    timeout,
    additional: two,
    min_cap: 3
  });
  assert_eq!(WriteConsistency::All { timeout }, WriteConsistency::All { timeout });
}
