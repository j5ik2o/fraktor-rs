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
  /// Key for `batch`.
  pub const BATCH: Self = Self::new("batch");
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
  /// Key for `drop`.
  pub const DROP: Self = Self::new("drop");
  /// Key for `drop_while`.
  pub const DROP_WHILE: Self = Self::new("drop_while");
  /// Key for `empty`.
  pub const EMPTY: Self = Self::new("empty");
  /// Key for `filter`.
  pub const FILTER: Self = Self::new("filter");
  /// Key for `filter_not`.
  pub const FILTER_NOT: Self = Self::new("filter_not");
  /// Key for `flatten_optional`.
  pub const FLATTEN_OPTIONAL: Self = Self::new("flatten_optional");
  /// Key for `flat_map_concat`.
  pub const FLAT_MAP_CONCAT: Self = Self::new("flat_map_concat");
  /// Key for `flat_map_merge`.
  pub const FLAT_MAP_MERGE: Self = Self::new("flat_map_merge");
  /// Key for `from_array`.
  pub const FROM_ARRAY: Self = Self::new("from_array");
  /// Key for `from_iterator`.
  pub const FROM_ITERATOR: Self = Self::new("from_iterator");
  /// Key for `from_option`.
  pub const FROM_OPTION: Self = Self::new("from_option");
  /// Key for `grouped`.
  pub const GROUPED: Self = Self::new("grouped");
  /// Key for `group_by`.
  pub const GROUP_BY: Self = Self::new("group_by");
  /// Key for `intersperse`.
  pub const INTERSPERSE: Self = Self::new("intersperse");
  /// Key for `map_async`.
  pub const MAP_ASYNC: Self = Self::new("map_async");
  /// Key for `map_concat`.
  pub const MAP_CONCAT: Self = Self::new("map_concat");
  /// Key for `map_option`.
  pub const MAP_OPTION: Self = Self::new("map_option");
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
  /// Key for `scan`.
  pub const SCAN: Self = Self::new("scan");
  /// Key for `shared_kill_switch`.
  pub const SHARED_KILL_SWITCH: Self = Self::new("shared_kill_switch");
  /// Key for `sliding`.
  pub const SLIDING: Self = Self::new("sliding");
  /// Key for `split_after`.
  pub const SPLIT_AFTER: Self = Self::new("split_after");
  /// Key for `split_when`.
  pub const SPLIT_WHEN: Self = Self::new("split_when");
  /// Key for `stateful_map`.
  pub const STATEFUL_MAP: Self = Self::new("stateful_map");
  /// Key for `stateful_map_concat`.
  pub const STATEFUL_MAP_CONCAT: Self = Self::new("stateful_map_concat");
  /// Key for `supervision`.
  pub const SUPERVISION: Self = Self::new("supervision");
  /// Key for `take`.
  pub const TAKE: Self = Self::new("take");
  /// Key for `take_until`.
  pub const TAKE_UNTIL: Self = Self::new("take_until");
  /// Key for `take_while`.
  pub const TAKE_WHILE: Self = Self::new("take_while");
  /// Key for `throttle`.
  pub const THROTTLE: Self = Self::new("throttle");
  /// Key for `unique_kill_switch`.
  pub const UNIQUE_KILL_SWITCH: Self = Self::new("unique_kill_switch");
  /// Key for `zip`.
  pub const ZIP: Self = Self::new("zip");
  /// Key for `zip_with_index`.
  pub const ZIP_WITH_INDEX: Self = Self::new("zip_with_index");

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
