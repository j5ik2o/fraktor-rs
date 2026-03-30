use crate::core::{StreamError, SupervisionStrategy};

#[cfg(test)]
mod tests;

/// Function signature used to decide supervision strategy from an observed error.
#[allow(dead_code)]
pub(in crate::core) type Decider = fn(&StreamError) -> SupervisionStrategy;
