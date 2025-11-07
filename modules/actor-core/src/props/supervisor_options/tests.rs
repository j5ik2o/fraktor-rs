use super::SupervisorOptions;
use crate::supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind};

#[test]
fn supervisor_options_new() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, core::time::Duration::from_secs(2), |_| {
      SupervisorDirective::Restart
    });
  let options = SupervisorOptions::new(strategy);
  assert_eq!(options.strategy().kind(), SupervisorStrategyKind::OneForOne);
}

#[test]
fn supervisor_options_strategy() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::AllForOne, 10, core::time::Duration::from_millis(500), |_| {
      SupervisorDirective::Stop
    });
  let options = SupervisorOptions::new(strategy);
  let returned_strategy = options.strategy();
  assert_eq!(returned_strategy.kind(), SupervisorStrategyKind::AllForOne);
}

#[test]
fn supervisor_options_default() {
  let options = SupervisorOptions::default();
  let strategy = options.strategy();
  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
}

#[test]
fn supervisor_options_clone() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, core::time::Duration::from_secs(1), |_| {
      SupervisorDirective::Restart
    });
  let options1 = SupervisorOptions::new(strategy);
  let options2 = options1.clone();
  assert_eq!(options1.strategy().kind(), options2.strategy().kind());
}

#[test]
fn supervisor_options_debug() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, core::time::Duration::from_secs(1), |_| {
      SupervisorDirective::Restart
    });
  let options = SupervisorOptions::new(strategy);
  fn assert_debug<T: core::fmt::Debug>(_t: &T) {}
  assert_debug(&options);
}
