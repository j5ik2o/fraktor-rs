use crate::core::typed::delivery::WorkerStats;

#[test]
fn worker_stats_accessors() {
  let stats = WorkerStats::new(5);
  assert_eq!(stats.number_of_workers(), 5);
}

#[test]
fn worker_stats_clone_and_eq() {
  let a = WorkerStats::new(3);
  let b = a;
  assert_eq!(a, b);
}
