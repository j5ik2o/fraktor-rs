//! Generic synchronized primitives.

mod guard_handle;
mod synchronized_mutex;
mod synchronized_mutex_backend;
mod synchronized_rw;
mod synchronized_rw_backend;

pub use guard_handle::GuardHandle;
pub use synchronized_mutex::Synchronized;
pub use synchronized_mutex_backend::SynchronizedMutexBackend;
pub use synchronized_rw::SynchronizedRw;
pub use synchronized_rw_backend::SynchronizedRwBackend;
