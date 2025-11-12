//! Actor selection expression and relative path resolution.

pub mod resolver;

#[cfg(test)]
mod tests;

pub use self::resolver::ActorSelectionResolver;
