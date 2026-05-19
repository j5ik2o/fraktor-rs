//! Typed path pattern for routing large messages.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum PatternSegment {
  Literal(String),
  MultiSegmentWildcard,
}

/// Actor path pattern that marks a destination as large-message eligible.
///
/// The pattern syntax follows the relative actor-path examples used by Pekko
/// Artery's `large-message-destinations` setting:
///
/// - `/user/large` for an exact path
/// - `/user/group/*` for a single-segment wildcard
/// - `/user/group/**` for a recursive wildcard
/// - `/temp/session-ask-actor*` for an in-segment wildcard
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LargeMessageDestinationPattern {
  pattern:  String,
  segments: Vec<PatternSegment>,
}

impl LargeMessageDestinationPattern {
  /// Creates a new path pattern.
  ///
  /// # Panics
  ///
  /// Panics when `pattern` is not an absolute actor path.
  #[must_use]
  pub fn new(pattern: impl Into<String>) -> Self {
    let pattern = pattern.into();
    let segments = parse_pattern_segments(&pattern);
    Self { pattern, segments }
  }

  /// Returns the original pattern string.
  #[must_use]
  pub fn pattern(&self) -> &str {
    &self.pattern
  }

  /// Returns `true` when `path` matches this pattern.
  ///
  /// # Panics
  ///
  /// Panics when `path` is not an absolute actor path.
  #[must_use]
  pub fn matches_absolute_path(&self, path: &str) -> bool {
    let path_segments = parse_candidate_segments(path);
    matches_segments(&self.segments, &path_segments)
  }
}

fn parse_pattern_segments(pattern: &str) -> Vec<PatternSegment> {
  let segments = parse_absolute_segments(pattern, "large-message destination pattern");
  segments
    .into_iter()
    .map(|segment| {
      if segment == "**" { PatternSegment::MultiSegmentWildcard } else { PatternSegment::Literal(segment.to_string()) }
    })
    .collect()
}

fn parse_candidate_segments(path: &str) -> Vec<&str> {
  parse_absolute_segments(path, "actor path")
}

fn parse_absolute_segments<'a>(path: &'a str, label: &str) -> Vec<&'a str> {
  assert!(path.starts_with('/'), "{label} must start with '/'");
  assert!(path != "/", "{label} must contain at least one path segment");
  let trimmed = path.trim_end_matches('/');
  let segments: Vec<&str> = trimmed.split('/').skip(1).collect();
  assert!(
    !segments.is_empty() && segments.iter().all(|segment| !segment.is_empty()),
    "{label} must not contain empty path segments",
  );
  segments
}

fn matches_segments(pattern: &[PatternSegment], path: &[&str]) -> bool {
  match pattern.split_first() {
    | None => path.is_empty(),
    | Some((PatternSegment::MultiSegmentWildcard, tail)) => {
      if tail.is_empty() {
        return true;
      }
      (0..=path.len()).any(|skip| matches_segments(tail, &path[skip..]))
    },
    | Some((PatternSegment::Literal(pattern_segment), tail)) => match path.split_first() {
      | Some((path_segment, path_tail)) => {
        segment_matches(pattern_segment, path_segment) && matches_segments(tail, path_tail)
      },
      | None => false,
    },
  }
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
  let pattern = pattern.as_bytes();
  let segment = segment.as_bytes();
  let mut pattern_index = 0;
  let mut segment_index = 0;
  let mut last_star = None;
  let mut retry_segment_index = 0;

  while segment_index < segment.len() {
    if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
      last_star = Some(pattern_index);
      pattern_index += 1;
      retry_segment_index = segment_index;
      continue;
    }

    if pattern_index < pattern.len() && pattern[pattern_index] == segment[segment_index] {
      pattern_index += 1;
      segment_index += 1;
      continue;
    }

    match last_star {
      | Some(star_index) => {
        pattern_index = star_index + 1;
        retry_segment_index += 1;
        segment_index = retry_segment_index;
      },
      | None => return false,
    }
  }

  while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
    pattern_index += 1;
  }

  pattern_index == pattern.len()
}
