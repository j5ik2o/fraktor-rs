//! Actor selection expression and relative path resolution.

mod actor_selection_error;
mod resolver;
mod selection;

#[cfg(test)]
mod tests;

pub use actor_selection_error::ActorSelectionError;
pub use resolver::ActorSelectionResolver;
pub use selection::ActorSelection;
