/// Stable key for a compatibility operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperatorKey {
  name: &'static str,
}

impl OperatorKey {
  /// Key for `async_boundary`.
  pub const ASYNC_BOUNDARY: Self = Self::new("async_boundary");
  /// Key for `balance`.
  pub const BALANCE: Self = Self::new("balance");
  /// Key for `broadcast`.
  pub const BROADCAST: Self = Self::new("broadcast");
  /// Key for `broadcast_hub`.
  pub const BROADCAST_HUB: Self = Self::new("broadcast_hub");
  /// Key for `buffer`.
  pub const BUFFER: Self = Self::new("buffer");
  /// Key for `concat`.
  pub const CONCAT: Self = Self::new("concat");
  /// Key for `concat_substreams`.
  pub const CONCAT_SUBSTREAMS: Self = Self::new("concat_substreams");
  /// Key for `flat_map_concat`.
  pub const FLAT_MAP_CONCAT: Self = Self::new("flat_map_concat");
  /// Key for `flat_map_merge`.
  pub const FLAT_MAP_MERGE: Self = Self::new("flat_map_merge");
  /// Key for `group_by`.
  pub const GROUP_BY: Self = Self::new("group_by");
  /// Key for `merge`.
  pub const MERGE: Self = Self::new("merge");
  /// Key for `merge_hub`.
  pub const MERGE_HUB: Self = Self::new("merge_hub");
  /// Key for `merge_substreams`.
  pub const MERGE_SUBSTREAMS: Self = Self::new("merge_substreams");
  /// Key for `merge_substreams_with_parallelism`.
  pub const MERGE_SUBSTREAMS_WITH_PARALLELISM: Self = Self::new("merge_substreams_with_parallelism");
  /// Key for `partition_hub`.
  pub const PARTITION_HUB: Self = Self::new("partition_hub");
  /// Key for `recover`.
  pub const RECOVER: Self = Self::new("recover");
  /// Key for `recover_with_retries`.
  pub const RECOVER_WITH_RETRIES: Self = Self::new("recover_with_retries");
  /// Key for `restart`.
  pub const RESTART: Self = Self::new("restart");
  /// Key for `shared_kill_switch`.
  pub const SHARED_KILL_SWITCH: Self = Self::new("shared_kill_switch");
  /// Key for `split_after`.
  pub const SPLIT_AFTER: Self = Self::new("split_after");
  /// Key for `split_when`.
  pub const SPLIT_WHEN: Self = Self::new("split_when");
  /// Key for `supervision`.
  pub const SUPERVISION: Self = Self::new("supervision");
  /// Key for `unique_kill_switch`.
  pub const UNIQUE_KILL_SWITCH: Self = Self::new("unique_kill_switch");
  /// Key for `zip`.
  pub const ZIP: Self = Self::new("zip");

  /// Creates an operator key from a stable operator name.
  #[must_use]
  pub const fn new(name: &'static str) -> Self {
    Self { name }
  }

  /// Returns the stable operator name.
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    self.name
  }
}
