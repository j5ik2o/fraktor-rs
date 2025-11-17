use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// DeadLetter store specialised for `StdToolbox`.
pub type DeadLetter = crate::core::dead_letter::DeadLetterGeneric<StdToolbox>;

/// Captures a single deadletter occurrence.
pub type DeadLetterEntry = crate::core::dead_letter::DeadLetterEntryGeneric<StdToolbox>;
