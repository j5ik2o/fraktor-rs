//! Internal fused-operator implementations.

use alloc::boxed::Box;
use core::{
  any::TypeId,
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use crate::core::{
  FlowDefinition, SupervisionStrategy,
  attributes::Attributes,
  mat::MatCombine,
  shape::{Inlet, Outlet},
  stage::StageKind,
};

/// Async boundary logic.
mod async_boundary_logic;
/// Backpressure timeout logic.
mod backpressure_timeout_logic;
/// Balance (round-robin fan-out) logic.
mod balance_logic;
/// Broadcast (clone fan-out) logic.
mod broadcast_logic;
/// Buffer logic with overflow policy.
mod buffer_logic;
/// Completion timeout logic.
mod completion_timeout_logic;
/// Concat (ordered fan-in) logic.
mod concat_logic;
/// Concat-lazy and prepend-lazy compatibility logic.
mod concat_source_logic;
/// Conflate-with-seed logic.
mod conflate_with_seed_logic;
/// Coupled termination logic.
mod coupled_termination_logic;
/// Debounce logic.
mod debounce_logic;
/// Drop (skip first N) logic.
mod drop_logic;
/// Drop-while logic.
mod drop_while_logic;
/// Expand (extrapolation) logic.
mod expand_logic;
/// Filter logic.
mod filter_logic;
/// FlatMap concat logic.
mod flat_map_concat_logic;
/// FlatMap merge logic.
mod flat_map_merge_logic;
/// Flat-map-prefix logic.
mod flat_map_prefix_logic;
/// Flatten substreams logic.
mod flatten_substreams_logic;
/// Flatten substreams with parallelism logic.
mod flatten_substreams_with_parallelism_logic;
/// Group-by logic.
mod group_by_logic;
/// Grouped (batch) logic.
mod grouped_logic;
/// Grouped-weighted logic.
mod grouped_weighted_within_logic;
/// Grouped-within (batch with timeout) logic.
mod grouped_within_logic;
/// Idle timeout logic.
mod idle_timeout_logic;
/// Initial timeout logic.
mod initial_timeout_logic;
/// Interleave (round-robin fan-in) logic.
mod interleave_logic;
/// Intersperse logic.
mod intersperse_logic;
/// Kill-switch pass-through logic.
mod kill_switch_logic;
/// Lazy flow instantiation logic.
mod lazy_flow_logic;
/// Limit-weighted logic.
mod limit_weighted_logic;
/// Logging pass-through logic.
mod log_logic;
/// Map-async logic.
mod map_async_logic;
/// Map-async partitioned logic.
mod map_async_partitioned_logic;
/// Map-concat logic.
mod map_concat_logic;
/// Failure-mapping logic.
mod map_error_logic;
/// Map logic.
mod map_logic;
/// Map-option logic.
mod map_option_logic;
/// Merge-latest logic.
mod merge_latest_logic;
/// Merge (unordered fan-in) logic.
mod merge_logic;
/// Merge-preferred logic.
mod merge_preferred_logic;
/// Merge-prioritized logic.
mod merge_prioritized_logic;
/// Merge-sorted logic.
mod merge_sorted_logic;
/// Conditional on-error-complete logic.
mod on_error_complete_logic;
/// Conditional on-error-continue logic.
mod on_error_continue_logic;
/// Partition logic.
mod partition_logic;
/// Prefix-and-tail logic.
mod prefix_and_tail_logic;
/// Recover logic.
mod recover_logic;
/// Recover-with-retries logic.
mod recover_with_retries_logic;
/// Retry flow logic with exponential backoff.
mod retry_flow_logic;
/// Sample logic.
mod sample_logic;
/// Scan logic.
mod scan_logic;
/// Sliding window logic.
mod sliding_logic;
/// Split-after logic.
mod split_after_logic;
/// Split-when logic.
mod split_when_logic;
/// Stateful map-concat accumulator logic.
mod stateful_map_concat_accumulator_logic;
/// Stateful map-concat logic.
mod stateful_map_concat_logic;
/// Stateful map logic.
mod stateful_map_logic;
/// Stateful map with on-complete logic.
mod stateful_map_with_on_complete_logic;
/// Strategy-based per-element delay logic.
mod strategy_delay_logic;
/// Take (first N) logic.
mod take_logic;
/// Take-until logic.
mod take_until_logic;
/// Take-while logic.
mod take_while_logic;
/// Take-within (time-bounded) logic.
mod take_within_logic;
/// Timed delay logic.
mod timed_delay_logic;
/// Fallible map-concat logic.
#[cfg(feature = "compression")]
mod try_map_concat_logic;
/// Unzip logic.
mod unzip_logic;
/// Unzip-with logic.
mod unzip_with_logic;
/// Watch-termination logic.
mod watch_termination_logic;
/// Wire-tap (fire-and-forget side output) logic.
mod wire_tap_logic;
/// Zip-all logic.
mod zip_all_logic;
/// Zip logic.
mod zip_logic;
/// Zip-with-index logic.
mod zip_with_index_logic;

pub(in crate::core) use async_boundary_logic::*;
pub(in crate::core) use backpressure_timeout_logic::*;
pub(in crate::core) use balance_logic::*;
pub(in crate::core) use broadcast_logic::*;
pub(in crate::core) use buffer_logic::*;
pub(in crate::core) use completion_timeout_logic::*;
pub(in crate::core) use concat_logic::*;
pub(in crate::core) use concat_source_logic::*;
pub(in crate::core) use conflate_with_seed_logic::*;
pub(in crate::core) use coupled_termination_logic::*;
pub(in crate::core) use debounce_logic::*;
pub(in crate::core) use drop_logic::*;
pub(in crate::core) use drop_while_logic::*;
pub(in crate::core) use expand_logic::*;
pub(in crate::core) use filter_logic::*;
pub(in crate::core) use flat_map_concat_logic::*;
pub(in crate::core) use flat_map_merge_logic::*;
pub(in crate::core) use flat_map_prefix_logic::*;
pub(in crate::core) use flatten_substreams_logic::*;
pub(in crate::core) use flatten_substreams_with_parallelism_logic::*;
pub(in crate::core) use group_by_logic::*;
pub(in crate::core) use grouped_logic::*;
pub(in crate::core) use grouped_weighted_within_logic::*;
pub(in crate::core) use grouped_within_logic::*;
pub(in crate::core) use idle_timeout_logic::*;
pub(in crate::core) use initial_timeout_logic::*;
pub(in crate::core) use interleave_logic::*;
pub(in crate::core) use intersperse_logic::*;
pub(in crate::core) use kill_switch_logic::*;
pub(in crate::core) use lazy_flow_logic::*;
pub(in crate::core) use limit_weighted_logic::*;
pub(in crate::core) use log_logic::*;
pub(in crate::core) use map_async_logic::*;
pub(in crate::core) use map_async_partitioned_logic::*;
pub(in crate::core) use map_concat_logic::*;
pub(in crate::core) use map_error_logic::*;
pub(in crate::core) use map_logic::*;
pub(in crate::core) use map_option_logic::*;
pub(in crate::core) use merge_latest_logic::*;
pub(in crate::core) use merge_logic::*;
pub(in crate::core) use merge_preferred_logic::*;
pub(in crate::core) use merge_prioritized_logic::*;
pub(in crate::core) use merge_sorted_logic::*;
pub(in crate::core) use on_error_complete_logic::*;
pub(in crate::core) use on_error_continue_logic::*;
pub(in crate::core) use partition_logic::*;
pub(in crate::core) use prefix_and_tail_logic::*;
pub(in crate::core) use recover_logic::*;
pub(in crate::core) use recover_with_retries_logic::*;
pub(in crate::core) use retry_flow_logic::*;
pub(in crate::core) use sample_logic::*;
pub(in crate::core) use scan_logic::*;
pub(in crate::core) use sliding_logic::*;
pub(in crate::core) use split_after_logic::*;
pub(in crate::core) use split_when_logic::*;
pub(in crate::core) use stateful_map_concat_accumulator_logic::*;
pub(in crate::core) use stateful_map_concat_logic::*;
pub(in crate::core) use stateful_map_logic::*;
pub(in crate::core) use stateful_map_with_on_complete_logic::*;
pub(in crate::core) use strategy_delay_logic::*;
pub(in crate::core) use take_logic::*;
pub(in crate::core) use take_until_logic::*;
pub(in crate::core) use take_while_logic::*;
pub(in crate::core) use take_within_logic::*;
pub(in crate::core) use timed_delay_logic::*;
#[cfg(feature = "compression")]
pub(in crate::core) use try_map_concat_logic::*;
pub(in crate::core) use unzip_logic::*;
pub(in crate::core) use unzip_with_logic::*;
pub(in crate::core) use watch_termination_logic::*;
pub(in crate::core) use wire_tap_logic::*;
pub(in crate::core) use zip_all_logic::*;
pub(in crate::core) use zip_logic::*;
pub(in crate::core) use zip_with_index_logic::*;

pub(in crate::core) const fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

const fn noop_raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)
}

const fn noop_clone(_: *const ()) -> RawWaker {
  noop_raw_waker()
}

const fn noop_wake(_: *const ()) {}

const fn noop_wake_by_ref(_: *const ()) {}

const fn noop_drop(_: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop_wake, noop_wake_by_ref, noop_drop);

pub(in crate::core) fn map_definition<In, Out, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapLogic { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::Left,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
    attributes:  Attributes::new(),
  }
}
