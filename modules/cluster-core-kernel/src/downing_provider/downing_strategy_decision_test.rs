use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::downing_provider::{
  DowningDecision, DowningDecisionTrace, DowningStrategyDecision, SplitBrainResolverStrategy,
};

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn keep_decision_records_partition_targets_and_trace_reason() {
  let retained = vec![unique("node-a", 1), unique("node-b", 2)];
  let targets = vec![unique("node-c", 3)];
  let trace = DowningDecisionTrace::majority_partition(
    SplitBrainResolverStrategy::KeepMajority,
    String::from("majority partition selected"),
  );

  let decision = DowningStrategyDecision::keep(trace, retained.clone(), targets.clone());

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::KeepMajority);
  assert_eq!(decision.trace().reason(), "majority partition selected");
  assert_eq!(decision.retained_partition(), retained.as_slice());
  assert_eq!(decision.downing_targets(), targets.as_slice());
  assert!(!decision.is_all_down());
}

#[test]
fn defer_decision_records_tie_break_and_stable_after_reason() {
  let trace = DowningDecisionTrace::stable_after_pending(
    SplitBrainResolverStrategy::StaticQuorum,
    Duration::from_secs(20),
    String::from("membership has not been stable long enough"),
  )
  .with_tie_break(String::from("same-size partitions defer"));

  let decision = DowningStrategyDecision::defer(trace);

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().stable_after_required(), Some(Duration::from_secs(20)));
  assert_eq!(decision.trace().tie_break_rule(), Some("same-size partitions defer"));
  assert!(decision.retained_partition().is_empty());
  assert!(decision.downing_targets().is_empty());
}

#[test]
fn all_down_decision_converts_to_down_and_records_timeout() {
  let targets = vec![unique("node-a", 1), unique("node-b", 2)];
  let trace = DowningDecisionTrace::down_all_elapsed(
    SplitBrainResolverStrategy::DownAll,
    Duration::from_secs(30),
    String::from("down-all timeout elapsed"),
  );

  let decision = DowningStrategyDecision::all_down(trace, targets.clone());

  assert_eq!(decision.simple_decision(), DowningDecision::Down);
  assert_eq!(decision.trace().down_all_timeout(), Some(Duration::from_secs(30)));
  assert_eq!(decision.downing_targets(), targets.as_slice());
  assert!(decision.retained_partition().is_empty());
  assert!(decision.is_all_down());
}
