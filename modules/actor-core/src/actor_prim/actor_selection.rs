//! Actor selection expression and relative path resolution.

mod actor_selection_error;
mod resolver;

#[cfg(test)]
mod tests;

pub use actor_selection_error::ActorSelectionError;
pub use resolver::ActorSelectionResolver;
