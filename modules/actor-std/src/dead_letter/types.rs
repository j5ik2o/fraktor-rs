use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// DeadLetter store specialised for `StdToolbox`.
pub type DeadLetter = cellactor_actor_core_rs::dead_letter::DeadLetter<StdToolbox>;
