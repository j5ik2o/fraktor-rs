//! Filesystem-backed snapshot store package.

mod local_snapshot_store;
mod local_snapshot_store_config;

pub use local_snapshot_store::LocalSnapshotStore;
pub use local_snapshot_store_config::LocalSnapshotStoreConfig;
