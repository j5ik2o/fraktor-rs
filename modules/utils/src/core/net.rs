//! Network utilities for URI parsing and validation.

mod uri_error;
mod uri_parser;
mod uri_parts;

pub use uri_error::UriError;
pub use uri_parser::UriParser;
pub use uri_parts::UriParts;
