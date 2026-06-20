use portable_atomic::Ordering;

use super::GuardianCellRegistry;
use crate::system::guardian::GuardianKind;

#[test]
fn guardian_cell_registry_starts_without_guardians() {
  let registry = GuardianCellRegistry::default();

  assert!(registry.guardians.pid(GuardianKind::Root).is_none());
  assert!(registry.guardians.pid(GuardianKind::System).is_none());
  assert!(registry.guardians.pid(GuardianKind::User).is_none());
  assert!(!registry.root_guardian_alive.load(Ordering::Acquire));
  assert!(!registry.system_guardian_alive.load(Ordering::Acquire));
  assert!(!registry.user_guardian_alive.load(Ordering::Acquire));
}
