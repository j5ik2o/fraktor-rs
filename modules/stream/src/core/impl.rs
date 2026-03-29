//! Internal implementation packages mirroring Pekko's `impl` boundary.

pub(in crate::core) mod fusing;
mod hub;
pub(in crate::core) mod interpreter;
mod io;
mod materialization;
mod queue;
mod streamref;

#[cfg(test)]
mod tests;
