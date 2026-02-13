//! Scheduler diagnostics subsystem types.

mod diagnostics_registry;
mod scheduler_diagnostics;
mod scheduler_diagnostics_event;
mod scheduler_diagnostics_subscription;

pub use scheduler_diagnostics::{SchedulerDiagnostics, SchedulerDiagnosticsGeneric};
pub use scheduler_diagnostics_event::SchedulerDiagnosticsEvent;
pub use scheduler_diagnostics_subscription::{
  SchedulerDiagnosticsSubscription, SchedulerDiagnosticsSubscriptionGeneric,
};
