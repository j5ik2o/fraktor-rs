//! Actor selection expression and relative path resolution.

use crate::{actor::actor_path::PathResolutionError, system::remote::RemoteAuthorityError};

mod actor_selection_error;
mod actor_selection_message;
mod resolver;
mod selection;
mod selection_path_element;

#[cfg(test)]
#[path = "actor_selection_test.rs"]
mod tests;

pub use actor_selection_error::ActorSelectionError;
pub use actor_selection_message::ActorSelectionMessage;
pub use resolver::ActorSelectionResolver;
pub use selection::ActorSelection;
pub use selection_path_element::SelectionPathElement;

const fn remote_authority_error_to_path_resolution(error: RemoteAuthorityError) -> PathResolutionError {
  match error {
    | RemoteAuthorityError::Quarantined => PathResolutionError::AuthorityQuarantined,
    | RemoteAuthorityError::DeferredQueueFull => PathResolutionError::AuthorityDeferredQueueFull,
  }
}
