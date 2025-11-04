use cellactor_utils_std_rs::StdToolbox;

/// DeadLetter store specialised for `StdToolbox`.
pub type DeadLetter = cellactor_actor_core_rs::dead_letter::DeadLetterGeneric<StdToolbox>;
