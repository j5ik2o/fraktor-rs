//! Validated actor path segments.

use alloc::string::String;

use super::ActorPathError;

const ALLOWED_PUNCTUATION: &[char] = &['-', '_', '.', '*', '+', ':', '@', '&', '=', ',', '!', '~', '\'', ';'];

/// Represents a validated actor path segment.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PathSegment {
  raw: String,
}

impl PathSegment {
  /// Creates a new validated path segment.
  pub fn new(value: impl Into<String>) -> Result<Self, ActorPathError> {
    let owned = value.into();
    validate_segment(&owned)?;
    Ok(Self { raw: owned })
  }

  /// Returns the segment as `&str`.
  #[must_use]
  pub fn as_str(&self) -> &str {
    &self.raw
  }
}

fn validate_segment(segment: &str) -> Result<(), ActorPathError> {
  if segment.is_empty() {
    return Err(ActorPathError::EmptySegment);
  }
  if segment.starts_with('$') {
    return Err(ActorPathError::ReservedSegment);
  }
  let mut chars = segment.chars().peekable();
  let mut idx = 0usize;
  while let Some(ch) = chars.next() {
    if ch == '%' {
      let hi = chars.next().ok_or(ActorPathError::InvalidPercentEncoding)?;
      let lo = chars.next().ok_or(ActorPathError::InvalidPercentEncoding)?;
      if !hi.is_ascii_hexdigit() || !lo.is_ascii_hexdigit() {
        return Err(ActorPathError::InvalidPercentEncoding);
      }
      idx += 3;
      continue;
    }
    if ch.is_ascii_alphanumeric() || ALLOWED_PUNCTUATION.contains(&ch) {
      idx += 1;
      continue;
    }
    return Err(ActorPathError::InvalidSegmentChar { ch, index: idx });
  }
  Ok(())
}
