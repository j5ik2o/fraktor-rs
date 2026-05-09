use crate::r#impl::{
  OperatorCatalog, OperatorContract, OperatorCoverage, OperatorKey, StreamDslError,
  default_operator_catalog_failure as failure, default_operator_catalog_fan_in as fan_in,
  default_operator_catalog_fan_out as fan_out, default_operator_catalog_hub as hub,
  default_operator_catalog_kill_switch as kill_switch, default_operator_catalog_source as source,
  default_operator_catalog_substream as substream, default_operator_catalog_timing as timing,
  default_operator_catalog_transform as transform,
};

#[cfg(test)]
mod tests;

const OPERATOR_COUNT: usize = 57;

const COVERAGE: [OperatorCoverage; OPERATOR_COUNT] = [
  source::coverage()[0],
  source::coverage()[1],
  source::coverage()[2],
  source::coverage()[3],
  transform::coverage()[0],
  transform::coverage()[1],
  transform::coverage()[2],
  transform::coverage()[3],
  transform::coverage()[4],
  transform::coverage()[5],
  transform::coverage()[6],
  transform::coverage()[7],
  transform::coverage()[8],
  transform::coverage()[9],
  transform::coverage()[10],
  transform::coverage()[11],
  transform::coverage()[12],
  transform::coverage()[13],
  transform::coverage()[14],
  transform::coverage()[15],
  transform::coverage()[16],
  transform::coverage()[17],
  transform::coverage()[18],
  substream::coverage()[0],
  substream::coverage()[1],
  substream::coverage()[2],
  substream::coverage()[3],
  substream::coverage()[4],
  substream::coverage()[5],
  substream::coverage()[6],
  substream::coverage()[7],
  timing::coverage()[0],
  timing::coverage()[1],
  timing::coverage()[2],
  timing::coverage()[3],
  timing::coverage()[4],
  fan_in::coverage()[0],
  fan_in::coverage()[1],
  fan_in::coverage()[2],
  fan_in::coverage()[3],
  fan_in::coverage()[4],
  fan_in::coverage()[5],
  fan_in::coverage()[6],
  fan_out::coverage()[0],
  fan_out::coverage()[1],
  fan_out::coverage()[2],
  fan_out::coverage()[3],
  fan_out::coverage()[4],
  failure::coverage()[0],
  failure::coverage()[1],
  failure::coverage()[2],
  failure::coverage()[3],
  hub::coverage()[0],
  hub::coverage()[1],
  hub::coverage()[2],
  kill_switch::coverage()[0],
  kill_switch::coverage()[1],
];

/// Default operator catalog for stream DSL.
pub struct DefaultOperatorCatalog;

impl DefaultOperatorCatalog {
  /// Creates a new default catalog.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for DefaultOperatorCatalog {
  fn default() -> Self {
    Self::new()
  }
}

impl OperatorCatalog for DefaultOperatorCatalog {
  fn lookup(&self, key: OperatorKey) -> Result<OperatorContract, StreamDslError> {
    source::lookup(key)
      .or_else(|| transform::lookup(key))
      .or_else(|| substream::lookup(key))
      .or_else(|| timing::lookup(key))
      .or_else(|| fan_in::lookup(key))
      .or_else(|| fan_out::lookup(key))
      .or_else(|| failure::lookup(key))
      .or_else(|| hub::lookup(key))
      .or_else(|| kill_switch::lookup(key))
      .ok_or(StreamDslError::UnsupportedOperator { key })
  }

  fn coverage(&self) -> &'static [OperatorCoverage] {
    &COVERAGE
  }
}

/// Converts an operator contract into coverage metadata.
pub(super) const fn coverage_for(contract: OperatorContract) -> OperatorCoverage {
  OperatorCoverage { key: contract.key, requirement_ids: contract.requirement_ids }
}
