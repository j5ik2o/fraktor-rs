/// Actor primitives specialised for the standard toolbox.
pub mod actor;
/// Dispatch bindings for the standard toolbox.
pub mod dispatch;
/// Event bindings for the standard toolbox.
pub mod event;
/// Pekko-inspired helper patterns for the standard toolbox.
pub mod pattern; // allow module_wiring::no_parent_reexport
pub use pattern::*;
/// Props and dispatcher configuration bindings for the standard toolbox.
pub mod props;
/// Scheduler bindings for the standard toolbox.
pub mod scheduler;
/// Actor system bindings for the standard toolbox.
pub mod system;
/// Typed actor utilities specialised for the standard toolbox runtime.
pub mod typed;

#[cfg(test)]
mod tests;
