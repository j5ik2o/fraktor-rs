use cellactor_utils_core_rs::sync::ToolboxMutex;

mod std_mutex_family;
mod std_toolbox;
#[cfg(test)]
mod tests;

pub use std_mutex_family::StdMutexFamily;
pub use std_toolbox::StdToolbox;

/// Convenience alias for the default std mutex.
pub type StdMutex<T> = ToolboxMutex<T, StdToolbox>;
