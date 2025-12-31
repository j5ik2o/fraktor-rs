use fraktor_actor_rs::core::{
  actor::Pid,
  system::{ActorSystemGeneric, SystemStateGeneric, SystemStateSharedGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_extension::PersistenceExtensionGeneric,
};

#[test]
fn persistence_extension_creates_actor_refs() {
  let state = SystemStateSharedGeneric::new(SystemStateGeneric::new());
  let system = ActorSystemGeneric::<NoStdToolbox>::from_state(state);
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();

  let extension = PersistenceExtensionGeneric::new(&system, journal, snapshot).expect("extension should build");

  assert_ne!(extension.journal_actor().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor().pid(), extension.snapshot_actor().pid());

  let cloned = extension.clone();
  assert_eq!(cloned.journal_actor().pid(), extension.journal_actor().pid());
}
