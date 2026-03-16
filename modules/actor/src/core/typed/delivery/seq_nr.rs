//! Sequence number type for reliable delivery.

#[cfg(test)]
mod tests;

/// Monotonically increasing sequence number for reliable delivery.
///
/// Starts at 1 and increments without gaps. Used to track message ordering,
/// confirmation, and resend requests between `ProducerController` and
/// `ConsumerController`.
pub type SeqNr = u64;
