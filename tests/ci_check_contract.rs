const CI_CHECK: &str = include_str!("../scripts/ci-check.sh");

#[test]
fn check_unit_sleep_scans_actor_typed_tests() {
  let body = check_unit_sleep_body();

  assert!(body.contains("modules/actor-core-typed/src/"), "check_unit_sleep must scan actor-typed unit tests");
  assert!(
    !body.contains("modules/actor-core-kernel/src/core/typed/"),
    "check_unit_sleep must not keep deleted actor-core typed allowlist paths"
  );
}

fn check_unit_sleep_body() -> &'static str {
  let start = CI_CHECK.find("check_unit_sleep()").expect("check_unit_sleep should exist");
  let end = CI_CHECK[start..].find("run_actor_path_e2e()").expect("run_actor_path_e2e should follow check_unit_sleep");
  &CI_CHECK[start..start + end]
}
