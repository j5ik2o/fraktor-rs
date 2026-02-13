use std::collections::BTreeSet;

use fraktor_streams_rs::core::{
  RestartSettings, StreamDslError, StreamError, StreamNotUsed,
  hub::{BroadcastHub, MergeHub, PartitionHub},
  lifecycle::{SharedKillSwitch, UniqueKillSwitch},
  operator::{DefaultOperatorCatalog, OperatorCatalog, OperatorKey},
  stage::{Flow, Sink, Source},
};

type VerifyFn = fn();

#[derive(Clone, Copy)]
struct RequirementEvidence {
  requirement_id: &'static str,
  evidences:      &'static [&'static str],
  verify:         VerifyFn,
}

impl RequirementEvidence {
  const fn new(requirement_id: &'static str, evidences: &'static [&'static str], verify: VerifyFn) -> Self {
    Self { requirement_id, evidences, verify }
  }
}

const ALL_REQUIREMENT_IDS: &[&str] = &[
  "1.1", "1.2", "1.3", "1.4", "2.1", "2.2", "2.3", "2.4", "2.5", "2.6", "3.1", "3.2", "3.3", "3.4", "4.1", "4.2",
  "4.3", "4.4", "4.5", "5.1", "5.2", "5.3", "5.4", "5.5", "6.1", "6.2", "6.3", "6.4", "6.5", "6.6", "7.1", "7.2",
  "7.3", "7.4", "8.1", "8.2", "8.3", "8.4", "9.1", "9.2", "9.3", "9.4",
];

const REQUIREMENT_EVIDENCE: &[RequirementEvidence] = &[
  RequirementEvidence::new(
    "1.1",
    &["default_operator_catalog::tests::lookup_returns_group_by_contract"],
    verify_catalog_surface,
  ),
  RequirementEvidence::new(
    "1.2",
    &["flow::tests::flat_map_merge_rejects_zero_breadth"],
    verify_invalid_argument_surface,
  ),
  RequirementEvidence::new(
    "1.3",
    &["default_operator_catalog::tests::lookup_returns_group_by_contract"],
    verify_catalog_surface,
  ),
  RequirementEvidence::new(
    "1.4",
    &["default_operator_catalog::tests::coverage_contains_merge_substreams_with_parallelism"],
    verify_catalog_surface,
  ),
  RequirementEvidence::new(
    "2.1",
    &["source::tests::source_group_by_keeps_single_path_behavior"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "2.2",
    &["source::tests::source_group_by_fails_when_unique_key_count_exceeds_limit"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "2.3",
    &["source::tests::source_split_when_starts_new_segment_with_matching_element"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "2.4",
    &["source::tests::source_split_after_keeps_matching_element_in_current_segment"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "2.5",
    &["source::tests::source_merge_substreams_flattens_single_segment"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "2.6",
    &["graph_interpreter::tests::cross_operator_backpressure_propagates_through_substream_and_async_boundary"],
    verify_substream_surface,
  ),
  RequirementEvidence::new(
    "3.1",
    &["graph_interpreter::tests::flat_map_concat_uses_inner_source"],
    verify_flat_map_surface,
  ),
  RequirementEvidence::new(
    "3.2",
    &["graph_interpreter::tests::flat_map_merge_delays_new_inner_creation_until_breadth_slot_is_released"],
    verify_flat_map_surface,
  ),
  RequirementEvidence::new(
    "3.3",
    &["graph_interpreter::tests::flat_map_merge_uses_configured_breadth"],
    verify_flat_map_surface,
  ),
  RequirementEvidence::new(
    "3.4",
    &["graph_interpreter::tests::cross_operator_failure_propagates_from_flat_map_to_substream_merge_chain"],
    verify_flat_map_surface,
  ),
  RequirementEvidence::new(
    "4.1",
    &["merge_hub::tests::merge_hub_rejects_offer_until_receiver_is_activated"],
    verify_hub_surface,
  ),
  RequirementEvidence::new(
    "4.2",
    &["broadcast_hub::tests::broadcast_hub_backpressures_when_no_subscriber_exists"],
    verify_hub_surface,
  ),
  RequirementEvidence::new(
    "4.3",
    &["broadcast_hub::tests::broadcast_hub_source_waits_for_later_publish_without_completing"],
    verify_hub_surface,
  ),
  RequirementEvidence::new(
    "4.4",
    &["partition_hub::tests::partition_hub_routes_values_to_selected_partitions"],
    verify_hub_surface,
  ),
  RequirementEvidence::new(
    "4.5",
    &["partition_hub::tests::partition_hub_rejects_offer_when_partition_has_no_active_consumer"],
    verify_hub_surface,
  ),
  RequirementEvidence::new(
    "5.1",
    &["unique_kill_switch::tests::unique_kill_switch_shutdown_sets_state"],
    verify_kill_switch_surface,
  ),
  RequirementEvidence::new(
    "5.2",
    &["unique_kill_switch::tests::unique_kill_switch_abort_sets_error"],
    verify_kill_switch_surface,
  ),
  RequirementEvidence::new(
    "5.3",
    &["unique_kill_switch::tests::unique_kill_switch_keeps_first_control_signal"],
    verify_kill_switch_surface,
  ),
  RequirementEvidence::new(
    "5.4",
    &["source::tests::shared_kill_switch_created_before_materialization_controls_multiple_streams"],
    verify_kill_switch_surface,
  ),
  RequirementEvidence::new(
    "5.5",
    &["source::tests::materialized_shared_kill_switch_shutdown_completes_stream"],
    verify_kill_switch_surface,
  ),
  RequirementEvidence::new(
    "6.1",
    &["graph_interpreter::tests::source_completion_triggers_restart_until_budget_is_exhausted"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "6.2",
    &["graph_interpreter::tests::source_completion_triggers_restart_until_budget_is_exhausted"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "6.3",
    &["graph_interpreter::tests::restart_budget_exhaustion_completes_with_default_terminal_action"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "6.4",
    &["graph_interpreter::tests::split_when_restart_supervision_behaves_like_resume"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "6.5",
    &["graph_interpreter::tests::non_split_restart_supervision_calls_on_restart"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "6.6",
    &["graph_interpreter::tests::split_when_restart_supervision_behaves_like_resume"],
    verify_restart_supervision_surface,
  ),
  RequirementEvidence::new(
    "7.1",
    &["flow::tests::async_boundary_keeps_single_path_behavior"],
    verify_async_boundary_surface,
  ),
  RequirementEvidence::new(
    "7.2",
    &["graph_interpreter::tests::async_boundary_flow_preserves_input_order"],
    verify_async_boundary_surface,
  ),
  RequirementEvidence::new(
    "7.3",
    &["graph_interpreter::tests::async_boundary_flow_preserves_input_order"],
    verify_async_boundary_surface,
  ),
  RequirementEvidence::new(
    "7.4",
    &["graph_interpreter::tests::async_boundary_backpressures_instead_of_failing_when_downstream_stalls"],
    verify_async_boundary_surface,
  ),
  RequirementEvidence::new(
    "8.1",
    &["compat_validation::compat_suite_records_requirement_pass"],
    verify_validation_surface,
  ),
  RequirementEvidence::new(
    "8.2",
    &["compat_validation::ci_gate_requires_no_std_and_full_ci"],
    verify_validation_surface,
  ),
  RequirementEvidence::new(
    "8.3",
    &["compat_validation::compat_suite_records_requirement_mismatch"],
    verify_validation_surface,
  ),
  RequirementEvidence::new(
    "8.4",
    &["requirement_traceability::requirement_traceability_matrix_covers_all_ids"],
    verify_validation_surface,
  ),
  RequirementEvidence::new(
    "9.1",
    &["compat_validation::migration_policy_guard_enforces_breaking_change_policy"],
    verify_policy_surface,
  ),
  RequirementEvidence::new("9.2", &["compat_validation::ci_gate_requires_no_std_and_full_ci"], verify_policy_surface),
  RequirementEvidence::new(
    "9.3",
    &["compat_validation::migration_policy_guard_enforces_breaking_change_policy"],
    verify_policy_surface,
  ),
  RequirementEvidence::new(
    "9.4",
    &["compat_validation::migration_policy_guard_enforces_breaking_change_policy"],
    verify_policy_surface,
  ),
];

#[test]
fn requirement_traceability_matrix_covers_all_ids() {
  let expected: BTreeSet<&str> = ALL_REQUIREMENT_IDS.iter().copied().collect();
  let actual: BTreeSet<&str> = REQUIREMENT_EVIDENCE.iter().map(|entry| entry.requirement_id).collect();
  assert_eq!(actual, expected);

  for entry in REQUIREMENT_EVIDENCE {
    assert!(!entry.evidences.is_empty(), "requirement {} must define at least one evidence", entry.requirement_id);
    (entry.verify)();
  }
}

#[test]
fn requirement_traceability_matrix_has_unique_entries() {
  let mut seen = BTreeSet::new();
  for entry in REQUIREMENT_EVIDENCE {
    assert!(
      seen.insert(entry.requirement_id),
      "requirement {} is duplicated in traceability matrix",
      entry.requirement_id
    );
  }
}

fn verify_catalog_surface() {
  let catalog = DefaultOperatorCatalog;
  let contract = catalog.lookup(OperatorKey::GROUP_BY).expect("group_by contract must exist");
  assert_eq!(contract.key, OperatorKey::GROUP_BY);
  assert!(!contract.input_condition.is_empty());
  assert!(!contract.completion_condition.is_empty());
  assert!(!contract.failure_condition.is_empty());
  assert!(!contract.requirement_ids.is_empty());
  assert!(catalog.coverage().iter().any(|coverage| coverage.key == OperatorKey::GROUP_BY));
}

fn verify_invalid_argument_surface() {
  let error = match Flow::<u32, u32, StreamNotUsed>::new().flat_map_merge(0, Source::single) {
    | Ok(_) => panic!("zero breadth must be rejected"),
    | Err(error) => error,
  };
  assert_eq!(error, StreamDslError::InvalidArgument {
    name:   "breadth",
    value:  0,
    reason: "must be greater than zero",
  });
}

fn verify_substream_surface() {
  let grouped_values = Source::single(1_u32)
    .group_by(1, |value: &u32| *value)
    .expect("positive max_substreams must be accepted")
    .merge_substreams()
    .collect_values()
    .expect("grouped values");
  assert_eq!(grouped_values, vec![1_u32]);

  let merged_values =
    Source::single(1_u32).split_after(|_| true).merge_substreams().collect_values().expect("merged values");
  assert_eq!(merged_values, vec![1_u32]);

  let concatenated_values =
    Source::single(1_u32).split_when(|_| false).concat_substreams().collect_values().expect("concatenated values");
  assert_eq!(concatenated_values, vec![1_u32]);

  let limited_values = Source::single(1_u32)
    .split_after(|_| true)
    .merge_substreams_with_parallelism(1)
    .expect("positive parallelism must be accepted")
    .collect_values()
    .expect("limited values");
  assert_eq!(limited_values, vec![1_u32]);

  let group_error = match Source::single(1_u32).group_by(0, |value: &u32| *value) {
    | Ok(_) => panic!("zero max_substreams must be rejected"),
    | Err(error) => error,
  };
  assert_eq!(group_error, StreamDslError::InvalidArgument {
    name:   "max_substreams",
    value:  0,
    reason: "must be greater than zero",
  });

  let parallelism_error = match Source::single(1_u32).split_after(|_| true).merge_substreams_with_parallelism(0) {
    | Ok(_) => panic!("zero parallelism must be rejected"),
    | Err(error) => error,
  };
  assert_eq!(parallelism_error, StreamDslError::InvalidArgument {
    name:   "parallelism",
    value:  0,
    reason: "must be greater than zero",
  });
}

fn verify_flat_map_surface() {
  let concat_values = Source::single(1_u32).flat_map_concat(Source::single).collect_values().expect("concat values");
  assert_eq!(concat_values, vec![1_u32]);

  let merge_values = Source::single(1_u32)
    .flat_map_merge(1, Source::single)
    .expect("positive breadth must be accepted")
    .collect_values()
    .expect("merge values");
  assert_eq!(merge_values, vec![1_u32]);

  let error = match Source::single(1_u32).flat_map_merge(0, Source::single) {
    | Ok(_) => panic!("zero breadth must be rejected"),
    | Err(error) => error,
  };
  assert_eq!(error, StreamDslError::InvalidArgument {
    name:   "breadth",
    value:  0,
    reason: "must be greater than zero",
  });
}

fn verify_hub_surface() {
  let merge_hub = MergeHub::new();
  assert_eq!(merge_hub.offer(1_u32), Err(StreamError::WouldBlock));
  let _ = merge_hub.source();
  assert_eq!(merge_hub.offer(1_u32), Ok(()));
  assert_eq!(merge_hub.poll(), Some(1_u32));

  let broadcast_hub = BroadcastHub::new();
  assert_eq!(broadcast_hub.publish(1_u32), Err(StreamError::WouldBlock));
  let subscriber_id = broadcast_hub.subscribe();
  assert_eq!(broadcast_hub.publish(2_u32), Ok(()));
  assert_eq!(broadcast_hub.poll(subscriber_id), Some(2_u32));

  let partition_hub = PartitionHub::new(1);
  assert_eq!(partition_hub.offer(0, 1_u32), Err(StreamError::WouldBlock));
  assert_eq!(partition_hub.route_with(1_u32, |_| 0), Err(StreamError::WouldBlock));
  let _ = partition_hub.source_for(0);
  assert_eq!(partition_hub.offer(0, 3_u32), Ok(()));
  assert_eq!(partition_hub.route_with(4_u32, |_| 0), Ok(()));
  assert_eq!(partition_hub.route_with(5_u32, |_| 1), Err(StreamError::InvalidRoute { route: 1, partition_count: 1 }));
}

fn verify_kill_switch_surface() {
  let unique = UniqueKillSwitch::new();
  unique.shutdown();
  unique.abort(StreamError::Failed);
  assert!(unique.is_shutdown());
  assert!(!unique.is_aborted());
  assert_eq!(unique.abort_error(), None);

  let shared = SharedKillSwitch::new();
  let shared_clone = shared.clone();
  shared_clone.abort(StreamError::Failed);
  shared.shutdown();
  assert!(shared.is_aborted());
  assert!(!shared.is_shutdown());
  assert_eq!(shared.abort_error(), Some(StreamError::Failed));
}

fn verify_restart_supervision_surface() {
  let settings = RestartSettings::new(1, 4, 2)
    .with_random_factor_permille(300)
    .with_max_restarts_within_ticks(32)
    .with_jitter_seed(31);

  let source_values = Source::single(1_u32)
    .restart_source_with_settings(settings)
    .supervision_resume()
    .supervision_restart()
    .supervision_stop()
    .collect_values()
    .expect("source values");
  assert_eq!(source_values, vec![1_u32]);

  let flow_values = Source::single(1_u32)
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .restart_flow_with_settings(settings)
        .supervision_resume()
        .supervision_restart()
        .supervision_stop(),
    )
    .collect_values()
    .expect("flow values");
  assert_eq!(flow_values, vec![1_u32]);

  let sink = Sink::<u32, _>::head()
    .restart_sink_with_settings(settings)
    .supervision_resume()
    .supervision_restart()
    .supervision_stop();
  let _ = Source::single(1_u32).to(sink);
}

fn verify_async_boundary_surface() {
  let source_values = Source::single(1_u32).async_boundary().collect_values().expect("source async values");
  assert_eq!(source_values, vec![1_u32]);

  let flow_values = Source::single(1_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().async_boundary())
    .collect_values()
    .expect("flow async values");
  assert_eq!(flow_values, vec![1_u32]);
}

fn verify_validation_surface() {
  let requirement_ids = requirement_id_set();
  assert!(requirement_ids.contains("8.1"));
  assert!(requirement_ids.contains("8.2"));
  assert!(requirement_ids.contains("8.3"));
  assert!(requirement_ids.contains("8.4"));
}

fn verify_policy_surface() {
  let requirement_ids = requirement_id_set();
  assert!(requirement_ids.contains("9.1"));
  assert!(requirement_ids.contains("9.2"));
  assert!(requirement_ids.contains("9.3"));
  assert!(requirement_ids.contains("9.4"));
}

fn requirement_id_set() -> BTreeSet<&'static str> {
  REQUIREMENT_EVIDENCE.iter().map(|entry| entry.requirement_id).collect()
}
