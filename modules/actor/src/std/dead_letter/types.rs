use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// DeadLetter store specialised for `StdToolbox` (shared wrapper).
pub type DeadLetter = crate::core::dead_letter::DeadLetterSharedGeneric<StdToolbox>;

/// Captures a single deadletter occurrence.
pub type DeadLetterEntry = crate::core::dead_letter::DeadLetterEntryGeneric<StdToolbox>;
