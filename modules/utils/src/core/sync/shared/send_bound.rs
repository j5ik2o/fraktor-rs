#[cfg(target_has_atomic = "ptr")]
/// Marker trait that enforces `Send` only when pointer atomics are available.
#[allow(dead_code)]
pub(crate) trait SendBound: Send {}

#[cfg(target_has_atomic = "ptr")]
impl<T: Send> SendBound for T {}

#[cfg(not(target_has_atomic = "ptr"))]
/// Marker trait for single-threaded targets where `Send` is not required.
#[allow(dead_code)]
pub(crate) trait SendBound {}

#[cfg(not(target_has_atomic = "ptr"))]
impl<T> SendBound for T {}
