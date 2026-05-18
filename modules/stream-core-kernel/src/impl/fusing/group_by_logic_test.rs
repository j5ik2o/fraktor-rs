use alloc::vec::Vec;
use core::marker::PhantomData;

use super::GroupByLogic;
use crate::{DownstreamCancelAction, FlowLogic, SubstreamCancelStrategy};

#[test]
fn group_by_downstream_cancel_uses_default_propagation() {
  let mut logic = GroupByLogic::<u32, u32, _> {
    max_substreams: 2,
    seen_keys: Vec::new(),
    key_fn: |value: &u32| *value % 2,
    substream_cancel_strategy: SubstreamCancelStrategy::Propagate,
    source_done: false,
    draining: false,
    _pd: PhantomData,
  };

  let action = FlowLogic::on_downstream_cancel(&mut logic).expect("downstream cancel");

  assert!(matches!(action, DownstreamCancelAction::Propagate));
}

#[test]
fn group_by_drain_strategy_requests_upstream_drain_after_downstream_cancel() {
  let mut logic = GroupByLogic::<u32, u32, _> {
    max_substreams: 2,
    seen_keys: Vec::new(),
    key_fn: |value: &u32| *value % 2,
    substream_cancel_strategy: SubstreamCancelStrategy::Drain,
    source_done: false,
    draining: false,
    _pd: PhantomData,
  };

  let action = FlowLogic::on_downstream_cancel(&mut logic).expect("downstream cancel");

  assert!(matches!(action, DownstreamCancelAction::Drain));
  assert!(FlowLogic::wants_upstream_drain(&logic));
}
