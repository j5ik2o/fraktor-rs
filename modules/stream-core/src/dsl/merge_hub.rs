use crate::r#impl::hub::MergeHub as CoreMergeHub;

/// Merge hub that fans in from a dynamic number of producers.
pub type MergeHub<T> = CoreMergeHub<T>;
