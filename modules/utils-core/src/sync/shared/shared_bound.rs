#[cfg(target_has_atomic = "ptr")]
/// Marker trait expressing synchronisation guarantees for shared handles.
pub trait SharedBound: Send + Sync {}

#[cfg(target_has_atomic = "ptr")]
impl<T: Send + Sync> SharedBound for T {}

#[cfg(not(target_has_atomic = "ptr"))]
/// Marker trait used when atomic pointer support is unavailable.
pub trait SharedBound {}

#[cfg(not(target_has_atomic = "ptr"))]
impl<T> SharedBound for T {}
