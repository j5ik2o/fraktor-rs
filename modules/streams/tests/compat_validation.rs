use fraktor_streams_rs::core::{BroadcastHub, OperatorKey, Source, StreamError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompatObservation {
  emits:         bool,
  backpressures: bool,
  completes:     bool,
  fails:         bool,
}

impl CompatObservation {
  const fn new(emits: bool, backpressures: bool, completes: bool, fails: bool) -> Self {
    Self { emits, backpressures, completes, fails }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompatReportEntry {
  requirement_id: &'static str,
  operator_key:   OperatorKey,
  expected:       CompatObservation,
  actual:         CompatObservation,
}

impl CompatReportEntry {
  const fn new(
    requirement_id: &'static str,
    operator_key: OperatorKey,
    expected: CompatObservation,
    actual: CompatObservation,
  ) -> Self {
    Self { requirement_id, operator_key, expected, actual }
  }
}

#[derive(Default)]
struct CompatReport {
  passed_cases: usize,
  mismatches:   Vec<CompatReportEntry>,
}

impl CompatReport {
  const fn new() -> Self {
    Self { passed_cases: 0, mismatches: Vec::new() }
  }

  fn record_pass(&mut self) {
    self.passed_cases = self.passed_cases.saturating_add(1);
  }

  fn record_mismatch(&mut self, entry: CompatReportEntry) {
    self.mismatches.push(entry);
  }
}

struct CompatSuite;

impl CompatSuite {
  const fn new() -> Self {
    Self
  }

  fn run_case(
    &self,
    requirement_id: &'static str,
    operator_key: OperatorKey,
    expected: CompatObservation,
    actual: CompatObservation,
    report: &mut CompatReport,
  ) -> bool {
    if expected == actual {
      report.record_pass();
      return true;
    }
    report.record_mismatch(CompatReportEntry::new(requirement_id, operator_key, expected, actual));
    false
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CiGateResult {
  Pass,
  Fail(&'static str),
}

struct CiGate;

impl CiGate {
  const fn new() -> Self {
    Self
  }

  const fn check_core_no_std(&self, passed: bool) -> CiGateResult {
    if passed { CiGateResult::Pass } else { CiGateResult::Fail("core no_std check failed") }
  }

  const fn check_full_ci(&self, passed: bool) -> CiGateResult {
    if passed { CiGateResult::Pass } else { CiGateResult::Fail("full CI check failed") }
  }

  const fn check_release_gate(&self, core_no_std_passed: bool, full_ci_passed: bool) -> CiGateResult {
    match self.check_core_no_std(core_no_std_passed) {
      | CiGateResult::Pass => self.check_full_ci(full_ci_passed),
      | failure => failure,
    }
  }
}

struct MigrationPolicyGuard;

impl MigrationPolicyGuard {
  const fn new() -> Self {
    Self
  }

  const fn allow_breaking_change(&self, required_for_compat_must: bool) -> bool {
    required_for_compat_must
  }

  const fn reject_legacy_guard(&self, has_legacy_guard: bool) -> bool {
    !has_legacy_guard
  }

  const fn validate_priority(&self, spec_compliant: bool, code_simple: bool) -> bool {
    spec_compliant && code_simple
  }
}

#[test]
fn compat_suite_records_requirement_mismatch() {
  let suite = CompatSuite::new();
  let mut report = CompatReport::new();
  let expected = CompatObservation::new(false, false, true, false);
  let actual = observe_broadcast_hub_backpressure();

  let passed = suite.run_case("8.3", OperatorKey::BUFFER, expected, actual, &mut report);

  assert!(!passed);
  assert_eq!(report.passed_cases, 0);
  assert_eq!(report.mismatches.len(), 1);
  assert_eq!(report.mismatches[0].requirement_id, "8.3");
  assert_eq!(report.mismatches[0].operator_key, OperatorKey::BUFFER);
}

#[test]
fn compat_suite_records_requirement_pass() {
  let suite = CompatSuite::new();
  let mut report = CompatReport::new();
  let observation = observe_group_by_merge_substreams();

  let passed = suite.run_case("8.1", OperatorKey::GROUP_BY, observation, observation, &mut report);

  assert!(passed);
  assert_eq!(report.passed_cases, 1);
  assert_eq!(report.mismatches.len(), 0);
}

#[test]
fn ci_gate_requires_no_std_and_full_ci() {
  let gate = CiGate::new();
  assert_eq!(gate.check_release_gate(true, true), CiGateResult::Pass);
  assert_eq!(gate.check_release_gate(false, true), CiGateResult::Fail("core no_std check failed"));
  assert_eq!(gate.check_release_gate(true, false), CiGateResult::Fail("full CI check failed"));
}

#[test]
fn migration_policy_guard_enforces_breaking_change_policy() {
  let guard = MigrationPolicyGuard::new();
  assert!(guard.allow_breaking_change(true));
  assert!(!guard.allow_breaking_change(false));
  assert!(guard.reject_legacy_guard(false));
  assert!(!guard.reject_legacy_guard(true));
  assert!(guard.validate_priority(true, true));
  assert!(!guard.validate_priority(false, true));
}

fn observe_group_by_merge_substreams() -> CompatObservation {
  match Source::single(7_u32)
    .group_by(4, |value: &u32| value % 2)
    .expect("group_by")
    .merge_substreams()
    .collect_values()
  {
    | Ok(values) => CompatObservation::new(!values.is_empty(), false, true, false),
    | Err(_) => CompatObservation::new(false, false, false, true),
  }
}

fn observe_broadcast_hub_backpressure() -> CompatObservation {
  let hub = BroadcastHub::new();
  match hub.publish(1_u32) {
    | Ok(_) => CompatObservation::new(true, false, false, false),
    | Err(StreamError::WouldBlock) => CompatObservation::new(false, true, false, false),
    | Err(_) => CompatObservation::new(false, false, false, true),
  }
}
