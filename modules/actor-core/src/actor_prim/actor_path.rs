//! Actor path primitives and formatting utilities.

pub mod error;
pub mod formatter;
pub mod parts;
pub mod path;
pub mod segment;
pub mod uid;

pub use error::{ActorPathError, PathResolutionError};
pub use formatter::ActorPathFormatter;
pub use parts::{ActorPathParts, GuardianKind};
pub use path::ActorPath;
pub use segment::PathSegment;
pub use uid::ActorUid;

#[cfg(test)]
mod tests;
