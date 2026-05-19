use super::OverflowStrategy;

#[test]
fn should_copy_strategy_variants() {
  let strategy = OverflowStrategy::DropBuffer;
  let copied = strategy;
  assert_eq!(copied, OverflowStrategy::DropBuffer);
}

#[test]
fn should_compare_strategy_variants() {
  assert_ne!(OverflowStrategy::DropHead, OverflowStrategy::DropTail);
  assert_eq!(OverflowStrategy::Fail, OverflowStrategy::Fail);
}

#[test]
fn should_expose_emit_early_variant_matching_pekko_parity() {
  // Given: Pekko exposes DelayOverflowStrategy.emitEarly as a first-class option.
  // When: constructing the EmitEarly variant.
  let strategy = OverflowStrategy::EmitEarly;

  // Then: it equals itself (Eq impl) and can be matched.
  assert_eq!(strategy, OverflowStrategy::EmitEarly);
}

#[test]
fn should_treat_emit_early_as_distinct_from_existing_variants() {
  // EmitEarly must not collide with any pre-existing strategy.
  assert_ne!(OverflowStrategy::EmitEarly, OverflowStrategy::Backpressure);
  assert_ne!(OverflowStrategy::EmitEarly, OverflowStrategy::DropHead);
  assert_ne!(OverflowStrategy::EmitEarly, OverflowStrategy::DropTail);
  assert_ne!(OverflowStrategy::EmitEarly, OverflowStrategy::DropBuffer);
  assert_ne!(OverflowStrategy::EmitEarly, OverflowStrategy::Fail);
}

#[test]
fn should_allow_emit_early_to_be_copied_and_cloned() {
  // OverflowStrategy derives Copy + Clone; EmitEarly must participate.
  let original = OverflowStrategy::EmitEarly;
  let copied = original;
  let cloned = original;
  assert_eq!(copied, OverflowStrategy::EmitEarly);
  assert_eq!(cloned, OverflowStrategy::EmitEarly);
}

#[test]
fn should_allow_emit_early_to_be_matched_exhaustively() {
  // Exhaustive match guarantees all call sites acknowledge the new variant.
  let strategy = OverflowStrategy::EmitEarly;
  let selected = match strategy {
    | OverflowStrategy::Backpressure => "backpressure",
    | OverflowStrategy::DropHead => "drop-head",
    | OverflowStrategy::DropTail => "drop-tail",
    | OverflowStrategy::DropBuffer => "drop-buffer",
    | OverflowStrategy::Fail => "fail",
    | OverflowStrategy::EmitEarly => "emit-early",
    | OverflowStrategy::DropNew => "drop-new",
  };
  assert_eq!(selected, "emit-early");
}

#[test]
fn should_expose_drop_new_variant_matching_pekko_parity() {
  // Given: Pekko exposes OverflowStrategy.dropNew as a first-class option that
  //        rejects the newly arrived element when the buffer is full.
  // When: constructing the DropNew variant.
  let strategy = OverflowStrategy::DropNew;

  // Then: it equals itself (Eq impl) and can be matched.
  assert_eq!(strategy, OverflowStrategy::DropNew);
}

#[test]
fn should_treat_drop_new_as_distinct_from_existing_variants() {
  // DropNew must not collide with any pre-existing strategy variant.
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::Backpressure);
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::DropHead);
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::DropTail);
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::DropBuffer);
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::Fail);
  assert_ne!(OverflowStrategy::DropNew, OverflowStrategy::EmitEarly);
}

#[test]
fn should_allow_drop_new_to_be_copied_and_cloned() {
  // OverflowStrategy derives Copy + Clone; DropNew must participate.
  let original = OverflowStrategy::DropNew;
  let copied = original;
  let cloned = original;
  assert_eq!(copied, OverflowStrategy::DropNew);
  assert_eq!(cloned, OverflowStrategy::DropNew);
}

#[test]
fn should_allow_drop_new_to_be_matched_exhaustively() {
  // Exhaustive match guarantees all call sites acknowledge the new variant.
  let strategy = OverflowStrategy::DropNew;
  let selected = match strategy {
    | OverflowStrategy::Backpressure => "backpressure",
    | OverflowStrategy::DropHead => "drop-head",
    | OverflowStrategy::DropTail => "drop-tail",
    | OverflowStrategy::DropBuffer => "drop-buffer",
    | OverflowStrategy::Fail => "fail",
    | OverflowStrategy::EmitEarly => "emit-early",
    | OverflowStrategy::DropNew => "drop-new",
  };
  assert_eq!(selected, "drop-new");
}
