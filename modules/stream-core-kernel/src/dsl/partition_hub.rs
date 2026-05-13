use crate::r#impl::hub::PartitionHub as CorePartitionHub;

/// Partition hub that routes elements to dynamic consumers by key.
pub type PartitionHub<T> = CorePartitionHub<T>;
