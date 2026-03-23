//! Compression facade providing gzip and deflate utilities.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use super::StreamError;

/// Compression and decompression facade for byte streams.
///
/// Provides associated functions for gzip and deflate operations
/// with configurable compression level, chunk size, and nowrap options.
pub struct Compression;

impl Compression {
  /// Default maximum bytes per decompressed chunk (64 KiB).
  pub const MAX_BYTES_PER_CHUNK_DEFAULT: usize = 64 * 1024;

  /// Compresses bytes using gzip format with default compression level (6).
  #[must_use]
  pub fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
    Self::gzip_bytes_with_level(bytes, 6)
  }

  /// Compresses bytes using gzip format with the specified compression level.
  #[must_use]
  pub fn gzip_bytes_with_level(bytes: &[u8], level: u8) -> Vec<u8> {
    let payload = Self::deflate_raw(bytes, level);
    let mut output = Vec::with_capacity(payload.len() + 18);
    output.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
    output.extend_from_slice(&payload);
    output.extend_from_slice(&crc32(bytes).to_le_bytes());
    // RFC 1952: ISIZE は入力サイズの mod 2^32。
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output
  }

  /// Decompresses gzip bytes with the default chunk size limit.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::CompressionError` if the data is invalid or too large.
  pub fn gunzip_bytes(bytes: &[u8]) -> Result<Vec<u8>, StreamError> {
    Self::gunzip_bytes_with_options(bytes, Self::MAX_BYTES_PER_CHUNK_DEFAULT)
  }

  /// Decompresses gzip bytes with an explicit maximum output size.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::CompressionError` if the data is invalid or exceeds
  /// `max_bytes_per_chunk`.
  pub fn gunzip_bytes_with_options(bytes: &[u8], max_bytes_per_chunk: usize) -> Result<Vec<u8>, StreamError> {
    if bytes.len() < 18 {
      return Err(StreamError::CompressionError { kind: "gzip_too_short" });
    }
    if bytes[0] != 0x1f || bytes[1] != 0x8b || bytes[2] != 0x08 {
      return Err(StreamError::CompressionError { kind: "gzip_header" });
    }
    let flags = bytes[3];
    if flags & 0b1110_0000 != 0 {
      return Err(StreamError::CompressionError { kind: "gzip_flags" });
    }
    let payload_end = bytes.len().saturating_sub(8);
    let mut payload_start = 10_usize;

    if flags & 0x04 != 0 {
      if payload_start + 2 > payload_end {
        return Err(StreamError::CompressionError { kind: "gzip_extra_len" });
      }
      let extra_len = u16::from_le_bytes([bytes[payload_start], bytes[payload_start + 1]]) as usize;
      payload_start += 2;
      if payload_start + extra_len > payload_end {
        return Err(StreamError::CompressionError { kind: "gzip_extra" });
      }
      payload_start += extra_len;
    }
    if flags & 0x08 != 0 {
      payload_start = consume_gzip_zero_terminated_field(bytes, payload_start, payload_end)?;
    }
    if flags & 0x10 != 0 {
      payload_start = consume_gzip_zero_terminated_field(bytes, payload_start, payload_end)?;
    }
    if flags & 0x02 != 0 {
      if payload_start + 2 > payload_end {
        return Err(StreamError::CompressionError { kind: "gzip_header_crc" });
      }
      payload_start += 2;
    }
    if payload_start > payload_end {
      return Err(StreamError::CompressionError { kind: "gzip_payload_bounds" });
    }

    let payload = &bytes[payload_start..payload_end];
    let expected_crc =
      u32::from_le_bytes([bytes[payload_end], bytes[payload_end + 1], bytes[payload_end + 2], bytes[payload_end + 3]]);
    let expected_len = u32::from_le_bytes([
      bytes[payload_end + 4],
      bytes[payload_end + 5],
      bytes[payload_end + 6],
      bytes[payload_end + 7],
    ]);
    if usize::try_from(expected_len).ok().filter(|len| *len <= max_bytes_per_chunk).is_none() {
      return Err(StreamError::CompressionError { kind: "gzip_too_large" });
    }
    let decompressed = Self::inflate_gzip_payload(payload, max_bytes_per_chunk)?;
    if crc32(&decompressed) != expected_crc || (decompressed.len() as u32) != expected_len {
      return Err(StreamError::CompressionError { kind: "gzip_trailer" });
    }
    Ok(decompressed)
  }

  /// Compresses bytes using raw deflate with default level (6).
  #[must_use]
  pub fn deflate_bytes(bytes: &[u8]) -> Vec<u8> {
    Self::deflate_raw(bytes, 6)
  }

  /// Compresses bytes using raw deflate with the specified level.
  #[must_use]
  pub fn deflate_bytes_with_level(bytes: &[u8], level: u8) -> Vec<u8> {
    Self::deflate_raw(bytes, level)
  }

  /// Compresses bytes with explicit level and nowrap option.
  ///
  /// When `nowrap` is `true`, raw deflate is used (no zlib header).
  /// When `nowrap` is `false`, zlib-wrapped format is used.
  #[must_use]
  pub fn deflate_bytes_with_options(bytes: &[u8], level: u8, nowrap: bool) -> Vec<u8> {
    if nowrap { Self::deflate_raw(bytes, level) } else { miniz_oxide::deflate::compress_to_vec_zlib(bytes, level) }
  }

  /// Decompresses raw deflate bytes with the default chunk limit.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::CompressionError` if the data is invalid.
  pub fn inflate_bytes(bytes: &[u8]) -> Result<Vec<u8>, StreamError> {
    Self::inflate_raw_with_limit(bytes, Self::MAX_BYTES_PER_CHUNK_DEFAULT)
  }

  /// Decompresses bytes with explicit max size and nowrap option.
  ///
  /// When `nowrap` is `true`, raw inflate is used.
  /// When `nowrap` is `false`, zlib-wrapped inflate is used.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::CompressionError` if the data is invalid or exceeds the limit.
  pub fn inflate_bytes_with_options(
    bytes: &[u8],
    max_bytes_per_chunk: usize,
    nowrap: bool,
  ) -> Result<Vec<u8>, StreamError> {
    if nowrap {
      Self::inflate_raw_with_limit(bytes, max_bytes_per_chunk)
    } else {
      Self::inflate_zlib_with_limit(bytes, max_bytes_per_chunk)
    }
  }

  // --- 内部ヘルパー ---

  fn deflate_raw(bytes: &[u8], level: u8) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(bytes, level)
  }

  fn inflate_gzip_payload(bytes: &[u8], max_bytes: usize) -> Result<Vec<u8>, StreamError> {
    let limit = max_bytes.saturating_add(1);
    let decompressed = miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, limit)
      .map_err(|_| StreamError::CompressionError { kind: "deflate" })?;
    if decompressed.len() > max_bytes {
      return Err(StreamError::CompressionError { kind: "gzip_too_large" });
    }
    Ok(decompressed)
  }

  fn inflate_raw_with_limit(bytes: &[u8], max_bytes: usize) -> Result<Vec<u8>, StreamError> {
    let limit = max_bytes.saturating_add(1);
    let decompressed = miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, limit)
      .map_err(|_| StreamError::CompressionError { kind: "deflate" })?;
    if decompressed.len() > max_bytes {
      return Err(StreamError::CompressionError { kind: "deflate_too_large" });
    }
    Ok(decompressed)
  }

  fn inflate_zlib_with_limit(bytes: &[u8], max_bytes: usize) -> Result<Vec<u8>, StreamError> {
    let limit = max_bytes.saturating_add(1);
    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, limit)
      .map_err(|_| StreamError::CompressionError { kind: "deflate" })?;
    if decompressed.len() > max_bytes {
      return Err(StreamError::CompressionError { kind: "deflate_too_large" });
    }
    Ok(decompressed)
  }
}

fn consume_gzip_zero_terminated_field(
  bytes: &[u8],
  mut index: usize,
  payload_end: usize,
) -> Result<usize, StreamError> {
  while index < payload_end {
    if bytes[index] == 0 {
      return Ok(index.saturating_add(1));
    }
    index = index.saturating_add(1);
  }
  Err(StreamError::CompressionError { kind: "gzip_string_field" })
}

fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = 0xffff_ffff_u32;
  for &byte in bytes {
    crc ^= u32::from(byte);
    for _ in 0..8 {
      let mask = (!((crc & 1).wrapping_sub(1))) & 0xedb8_8320;
      crc = (crc >> 1) ^ mask;
    }
  }
  !crc
}
