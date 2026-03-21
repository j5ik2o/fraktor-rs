use alloc::vec::Vec;
use core::marker::PhantomData;

use super::GroupByLogic;
use crate::core::{DownstreamCancelAction, FlowLogic};

#[test]
fn group_by_downstream_cancel_uses_default_propagation() {
  let mut logic = GroupByLogic::<u32, u32, _> {
    max_substreams: 2,
    seen_keys:      Vec::new(),
    key_fn:         |value: &u32| *value % 2,
    _pd:            PhantomData,
  };

  let action = FlowLogic::on_downstream_cancel(&mut logic).expect("downstream cancel");

  assert!(matches!(action, DownstreamCancelAction::Propagate));
}
