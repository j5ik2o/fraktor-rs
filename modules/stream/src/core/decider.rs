use super::{StreamError, SupervisionStrategy};

#[cfg(test)]
mod tests;

/// Function signature used to decide supervision strategy from an observed error.
pub type Decider = fn(&StreamError) -> SupervisionStrategy;
