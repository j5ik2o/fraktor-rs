use super::{TestSinkProbe, TestSourceProbe};

#[cfg(test)]
mod tests;

/// Deterministic fuzz runner for probe-based stream interaction tests.
pub struct StreamFuzzRunner {
  state: u64,
}

impl StreamFuzzRunner {
  /// Creates a new fuzz runner with deterministic seed.
  #[must_use]
  pub const fn new(seed: u64) -> Self {
    Self { state: seed }
  }

  const fn next_u32(&mut self) -> u32 {
    self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (self.state >> 32) as u32
  }

  /// Executes randomized source/sink interactions.
  pub fn run_source_sink_roundtrip(
    &mut self,
    source: &mut TestSourceProbe<u32>,
    sink: &mut TestSinkProbe<u32>,
    steps: usize,
  ) {
    for _ in 0..steps {
      match self.next_u32() % 3 {
        | 0 => source.push(self.next_u32() % 1000),
        | 1 => sink.request(1),
        | _ => {
          if let Some(value) = source.pull() {
            let _ = sink.push(value);
          }
        },
      }
    }
  }
}
