//! Sequence number type for reliable delivery.

#[cfg(test)]
#[path = "seq_nr_test.rs"]
mod tests;

/// Monotonically increasing sequence number for reliable delivery.
///
/// Starts at 1 and increments without gaps. Used to track message ordering,
/// confirmation, and resend requests between `ProducerController` and
/// `ConsumerController`.
pub type SeqNr = u64;
