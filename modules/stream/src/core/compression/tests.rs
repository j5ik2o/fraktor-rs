use alloc::vec::Vec;

use crate::core::Compression;

// --- gzip round-trip ---

#[test]
fn gzip_default_round_trips_bytes() {
  // Given: arbitrary input bytes
  let input: Vec<u8> = alloc::vec![1, 2, 3, 4, 5, 6, 7, 8];

  // When: compressing and then decompressing with defaults
  let compressed = Compression::gzip_bytes(&input);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // Then: output matches original input
  assert_eq!(decompressed, input);
}

#[test]
fn gzip_with_level_round_trips_bytes() {
  // Given: input bytes and a specific compression level
  let input: Vec<u8> = alloc::vec![10, 20, 30, 40, 50];

  // When: compressing with explicit level and decompressing
  let compressed = Compression::gzip_bytes_with_level(&input, 1);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // Then: output matches original input
  assert_eq!(decompressed, input);
}

#[test]
fn gzip_empty_input_round_trips() {
  // Given: empty input
  let input: Vec<u8> = Vec::new();

  // When: compressing and decompressing empty data
  let compressed = Compression::gzip_bytes(&input);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // Then: output is also empty
  assert_eq!(decompressed, input);
}

// --- deflate round-trip ---

#[test]
fn deflate_default_round_trips_bytes() {
  // Given: arbitrary input bytes
  let input: Vec<u8> = alloc::vec![100, 200, 150, 50, 0, 255];

  // When: compressing and decompressing with deflate defaults
  let compressed = Compression::deflate_bytes(&input);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // Then: output matches original input
  assert_eq!(decompressed, input);
}

#[test]
fn deflate_with_level_round_trips_bytes() {
  // Given: input bytes and specific compression level
  let input: Vec<u8> = alloc::vec![0, 1, 2, 3, 4, 5];

  // When: compressing with explicit level
  let compressed = Compression::deflate_bytes_with_level(&input, 9);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // Then: output matches original input
  assert_eq!(decompressed, input);
}

#[test]
fn deflate_empty_input_round_trips() {
  // Given: empty input
  let input: Vec<u8> = Vec::new();

  // When: compressing and decompressing empty data
  let compressed = Compression::deflate_bytes(&input);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // Then: output is also empty
  assert_eq!(decompressed, input);
}

// --- inflate error cases ---

#[test]
fn inflate_rejects_invalid_data() {
  // Given: invalid compressed data
  let invalid: Vec<u8> = alloc::vec![0xFF, 0xFE, 0xFD];

  // When: attempting to decompress invalid data
  let result = Compression::inflate_bytes(&invalid);

  // Then: returns an error
  assert!(result.is_err());
}

#[test]
fn inflate_with_max_bytes_rejects_oversized_output() {
  // Given: compressed data that would decompress to more than max_bytes_per_chunk
  let input: Vec<u8> = alloc::vec![0; 1024];
  let compressed = Compression::deflate_bytes(&input);

  // When: decompressing with a very small limit
  let result = Compression::inflate_bytes_with_options(&compressed, 16, false);

  // Then: returns an error because decompressed size exceeds limit
  assert!(result.is_err());
}

// --- gunzip error cases ---

#[test]
fn gunzip_rejects_truncated_header() {
  // Given: data too short to be valid gzip
  let short: Vec<u8> = alloc::vec![0x1f, 0x8b];

  // When: attempting to decompress
  let result = Compression::gunzip_bytes(&short);

  // Then: returns an error
  assert!(result.is_err());
}

#[test]
fn gunzip_rejects_invalid_magic_bytes() {
  // Given: data with wrong magic bytes
  let invalid: Vec<u8> = alloc::vec![
    0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
  ];

  // When: attempting to decompress
  let result = Compression::gunzip_bytes(&invalid);

  // Then: returns an error
  assert!(result.is_err());
}

// --- max_bytes_per_chunk_default constant ---

#[test]
fn max_bytes_per_chunk_default_is_64k() {
  // Then: default matches Pekko's MaxBytesPerChunkDefault
  assert_eq!(Compression::MAX_BYTES_PER_CHUNK_DEFAULT, 64 * 1024);
}

// --- nowrap option ---

#[test]
fn deflate_with_nowrap_round_trips() {
  // Given: input bytes
  let input: Vec<u8> = alloc::vec![42, 43, 44, 45];

  // When: compressing with nowrap=true and decompressing with nowrap=true
  let compressed = Compression::deflate_bytes_with_options(&input, 6, true);
  let decompressed =
    Compression::inflate_bytes_with_options(&compressed, Compression::MAX_BYTES_PER_CHUNK_DEFAULT, true)
      .expect("decompress");

  // Then: output matches original input
  assert_eq!(decompressed, input);
}

// --- decompress with explicit max_bytes ---

#[test]
fn gzip_decompress_with_max_bytes_accepts_within_limit() {
  // Given: small input that compresses to small output
  let input: Vec<u8> = alloc::vec![1, 2, 3];
  let compressed = Compression::gzip_bytes(&input);

  // When: decompressing with a generous limit
  let decompressed = Compression::gunzip_bytes_with_options(&compressed, 1024).expect("decompress");

  // Then: output matches original
  assert_eq!(decompressed, input);
}
