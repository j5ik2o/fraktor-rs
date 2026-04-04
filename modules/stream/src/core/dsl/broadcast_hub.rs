use crate::core::r#impl::hub::BroadcastHub as CoreBroadcastHub;

/// Broadcast hub that fans out to dynamic number of consumers.
pub type BroadcastHub<T> = CoreBroadcastHub<T>;
