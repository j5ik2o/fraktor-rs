//! Counter arithmetic error vocabulary.

/// Error returned when a counter operation exceeds its bounded representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterArithmeticError {
  /// The requested operation would overflow or exceed the signed result range.
  Overflow,
}
