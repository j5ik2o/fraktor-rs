use super::SchedulerCapacityProfile;

#[test]
fn tick_buffer_quota_scales_with_profile() {
  let tiny = SchedulerCapacityProfile::tiny();
  assert!(tiny.tick_buffer_quota() >= 32);

  let standard = SchedulerCapacityProfile::standard();
  assert!(standard.tick_buffer_quota() > tiny.tick_buffer_quota());
}
