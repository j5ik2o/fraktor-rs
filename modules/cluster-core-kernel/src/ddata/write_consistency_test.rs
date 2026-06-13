use core::time::Duration;

use crate::ddata::WriteConsistency;

#[test]
fn write_consistency_variants_keep_parameters() {
  let timeout = Duration::from_secs(5);

  assert_eq!(WriteConsistency::Local, WriteConsistency::Local);
  assert_eq!(WriteConsistency::To { n: 2, timeout }, WriteConsistency::To { n: 2, timeout });
  assert_eq!(WriteConsistency::Majority { timeout, min_cap: 3 }, WriteConsistency::Majority { timeout, min_cap: 3 });
  assert_eq!(WriteConsistency::MajorityPlus { timeout, additional: 2, min_cap: 3 }, WriteConsistency::MajorityPlus {
    timeout,
    additional: 2,
    min_cap: 3
  });
  assert_eq!(WriteConsistency::All { timeout }, WriteConsistency::All { timeout });
}
