//! Guardian and cell table state owned by SystemState.

#[cfg(test)]
#[path = "guardian_cell_registry_test.rs"]
mod tests;

use portable_atomic::AtomicBool;

use super::{CellsShared, GuardiansState, Registries};

/// Owns guardian state, actor cells, and name registries.
#[derive(Default)]
pub(crate) struct GuardianCellRegistry {
  pub(crate) cells:                 CellsShared,
  pub(crate) registries:            Registries,
  pub(crate) guardians:             GuardiansState,
  pub(crate) root_guardian_alive:   AtomicBool,
  pub(crate) system_guardian_alive: AtomicBool,
  pub(crate) user_guardian_alive:   AtomicBool,
}
