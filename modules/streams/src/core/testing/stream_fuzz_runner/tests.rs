use crate::core::testing::{StreamFuzzRunner, TestSinkProbe, TestSourceProbe};

#[test]
fn stream_fuzz_runner_is_deterministic_for_same_seed() {
  let mut source_a = TestSourceProbe::new();
  let mut sink_a = TestSinkProbe::new();
  let mut runner_a = StreamFuzzRunner::new(42);
  runner_a.run_source_sink_roundtrip(&mut source_a, &mut sink_a, 128);

  let mut source_b = TestSourceProbe::new();
  let mut sink_b = TestSinkProbe::new();
  let mut runner_b = StreamFuzzRunner::new(42);
  runner_b.run_source_sink_roundtrip(&mut source_b, &mut sink_b, 128);

  assert_eq!(sink_a.received(), sink_b.received());
}
