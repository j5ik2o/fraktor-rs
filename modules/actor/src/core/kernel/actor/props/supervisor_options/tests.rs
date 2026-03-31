use super::SupervisorOptions;
use crate::core::kernel::actor::supervision::{
  BackoffSupervisorStrategy, SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind,
};

#[test]
fn supervisor_options_from_strategy() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, core::time::Duration::from_secs(2), |_| {
      SupervisorDirective::Restart
    });
  let options = SupervisorOptions::from_strategy(strategy);
  match options.strategy() {
    | SupervisorStrategyConfig::Standard(s) => assert_eq!(s.kind(), SupervisorStrategyKind::OneForOne),
    | SupervisorStrategyConfig::Backoff(_) => panic!("expected Standard"),
  }
}

#[test]
fn supervisor_options_new_with_config() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::AllForOne, 10, core::time::Duration::from_millis(500), |_| {
      SupervisorDirective::Stop
    });
  let options = SupervisorOptions::new(SupervisorStrategyConfig::Standard(strategy));
  match options.strategy() {
    | SupervisorStrategyConfig::Standard(s) => assert_eq!(s.kind(), SupervisorStrategyKind::AllForOne),
    | SupervisorStrategyConfig::Backoff(_) => panic!("expected Standard"),
  }
}

#[test]
fn supervisor_options_with_backoff() {
  let backoff =
    BackoffSupervisorStrategy::new(core::time::Duration::from_millis(100), core::time::Duration::from_secs(10), 0.2);
  let options = SupervisorOptions::new(SupervisorStrategyConfig::Backoff(backoff));
  match options.strategy() {
    | SupervisorStrategyConfig::Standard(_) => panic!("expected Backoff"),
    | SupervisorStrategyConfig::Backoff(b) => {
      assert_eq!(b.min_backoff(), core::time::Duration::from_millis(100));
    },
  }
}

#[test]
fn supervisor_options_default() {
  let options = SupervisorOptions::default();
  match options.strategy() {
    | SupervisorStrategyConfig::Standard(s) => assert_eq!(s.kind(), SupervisorStrategyKind::OneForOne),
    | SupervisorStrategyConfig::Backoff(_) => panic!("expected Standard"),
  }
}

#[test]
fn supervisor_options_clone() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, core::time::Duration::from_secs(1), |_| {
      SupervisorDirective::Restart
    });
  let options1 = SupervisorOptions::from_strategy(strategy);
  let options2 = options1.clone();
  match (options1.strategy(), options2.strategy()) {
    | (SupervisorStrategyConfig::Standard(s1), SupervisorStrategyConfig::Standard(s2)) => {
      assert_eq!(s1.kind(), s2.kind());
    },
    | _ => panic!("expected both Standard"),
  }
}

#[test]
fn supervisor_options_debug() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, core::time::Duration::from_secs(1), |_| {
      SupervisorDirective::Restart
    });
  let options = SupervisorOptions::from_strategy(strategy);
  fn assert_debug<T: core::fmt::Debug>(_t: &T) {}
  assert_debug(&options);
}
