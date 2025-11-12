//! Actor path primitives and formatting utilities.

mod actor_path_error;
mod actor_path_parts;
mod actor_path_scheme;
mod comparator;
mod formatter;
mod guardian_kind;
mod parser;
mod path;
mod path_authority;
mod path_resolution_error;
mod segment;
mod uid;

pub use actor_path_error::ActorPathError;
pub use actor_path_parts::ActorPathParts;
pub use actor_path_scheme::ActorPathScheme;
pub use comparator::ActorPathComparator;
pub use formatter::ActorPathFormatter;
pub use guardian_kind::GuardianKind;
pub use parser::ActorPathParser;
pub use path::ActorPath;
pub(crate) use path_authority::PathAuthority;
pub use path_resolution_error::PathResolutionError;
pub use segment::PathSegment;
pub use uid::ActorUid;

#[cfg(test)]
mod tests;
