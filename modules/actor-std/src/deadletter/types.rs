use cellactor_utils_std_rs::StdToolbox;

/// Deadletter store specialised for `StdToolbox`.
pub type Deadletter = cellactor_actor_core_rs::deadletter::DeadletterGeneric<StdToolbox>;
