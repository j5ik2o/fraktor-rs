use alloc::{collections::VecDeque, vec::Vec};

use crate::core::testing::{StreamFuzzRunner, TestSinkProbe, TestSourceProbe};

fn next_u32(state: &mut u64) -> u32 {
  *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
  (*state >> 32) as u32
}

fn simulate_reference_roundtrip(seed: u64, steps: usize) -> Vec<u32> {
  let mut state = seed;
  let mut source_queue = VecDeque::new();
  let mut sink_demand = 0_usize;
  let mut received = Vec::new();

  for _ in 0..steps {
    match next_u32(&mut state) % 3 {
      | 0 => source_queue.push_back(next_u32(&mut state) % 1000),
      | 1 => sink_demand = sink_demand.saturating_add(1),
      | _ => {
        if let Some(value) = source_queue.pop_front()
          && sink_demand > 0
        {
          sink_demand = sink_demand.saturating_sub(1);
          received.push(value);
        }
      },
    }
  }
  received
}

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

#[test]
fn stream_fuzz_runner_matches_reference_model_under_stress() {
  let seed = 2026_u64;
  let steps = 4096_usize;
  let expected = simulate_reference_roundtrip(seed, steps);

  let mut source = TestSourceProbe::new();
  let mut sink = TestSinkProbe::new();
  let mut runner = StreamFuzzRunner::new(seed);
  runner.run_source_sink_roundtrip(&mut source, &mut sink, steps);

  assert_eq!(sink.received(), expected.as_slice());
}
