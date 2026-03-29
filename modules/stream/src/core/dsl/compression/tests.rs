use alloc::vec::Vec;

use crate::core::dsl::Compression;

// --- gzip ラウンドトリップ ---

#[test]
fn gzip_default_round_trips_bytes() {
  // 準備: 任意の入力バイト列
  let input: Vec<u8> = alloc::vec![1, 2, 3, 4, 5, 6, 7, 8];

  // 実行: デフォルト設定で圧縮・解凍
  let compressed = Compression::gzip_bytes(&input);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

#[test]
fn gzip_with_level_round_trips_bytes() {
  // 準備: 入力バイト列と圧縮レベル
  let input: Vec<u8> = alloc::vec![10, 20, 30, 40, 50];

  // 実行: 明示的レベルで圧縮・解凍
  let compressed = Compression::gzip_bytes_with_level(&input, 1);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

#[test]
fn gzip_empty_input_round_trips() {
  // 準備: 空の入力
  let input: Vec<u8> = Vec::new();

  // 実行: 空データの圧縮・解凍
  let compressed = Compression::gzip_bytes(&input);
  let decompressed = Compression::gunzip_bytes(&compressed).expect("decompress");

  // 検証: 出力も空
  assert_eq!(decompressed, input);
}

// --- deflate ラウンドトリップ ---

#[test]
fn deflate_default_round_trips_bytes() {
  // 準備: 任意の入力バイト列
  let input: Vec<u8> = alloc::vec![100, 200, 150, 50, 0, 255];

  // 実行: deflate デフォルト設定で圧縮・解凍
  let compressed = Compression::deflate_bytes(&input);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

#[test]
fn deflate_with_level_round_trips_bytes() {
  // 準備: 入力バイト列と圧縮レベル
  let input: Vec<u8> = alloc::vec![0, 1, 2, 3, 4, 5];

  // 実行: 明示的レベルで圧縮
  let compressed = Compression::deflate_bytes_with_level(&input, 9);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

#[test]
fn deflate_empty_input_round_trips() {
  // 準備: 空の入力
  let input: Vec<u8> = Vec::new();

  // 実行: 空データの圧縮・解凍
  let compressed = Compression::deflate_bytes(&input);
  let decompressed = Compression::inflate_bytes(&compressed).expect("decompress");

  // 検証: 出力も空
  assert_eq!(decompressed, input);
}

// --- inflate エラーケース ---

#[test]
fn inflate_rejects_invalid_data() {
  // 準備: 無効な圧縮データ
  let invalid: Vec<u8> = alloc::vec![0xFF, 0xFE, 0xFD];

  // 実行: 無効データの解凍を試行
  let result = Compression::inflate_bytes(&invalid);

  // 検証: deflate 解析エラーが返される
  assert!(matches!(result, Err(crate::core::stream_error::StreamError::CompressionError { kind: "deflate" })));
}

#[test]
fn inflate_with_max_bytes_rejects_oversized_output() {
  // 準備: max_bytes_per_chunk を超えるサイズに解凍されるデータ（nowrap=true で圧縮）
  let input: Vec<u8> = alloc::vec![0; 1024];
  let compressed = Compression::deflate_bytes_with_options(&input, 6, true);

  // 実行: 非常に小さい制限で解凍（nowrap=true で一致させる）
  let result = Compression::inflate_bytes_with_options(&compressed, 16, true);

  // 検証: 解凍サイズが制限を超えるためエラー
  assert!(result.is_err());
}

// --- gunzip エラーケース ---

#[test]
fn gunzip_rejects_truncated_header() {
  // 準備: 有効な gzip には短すぎるデータ
  let short: Vec<u8> = alloc::vec![0x1f, 0x8b];

  // 実行: 解凍を試行
  let result = Compression::gunzip_bytes(&short);

  // 検証: エラーが返される
  assert!(result.is_err());
}

#[test]
fn gunzip_rejects_invalid_magic_bytes() {
  // 準備: 不正なマジックバイトのデータ
  let invalid: Vec<u8> = alloc::vec![
    0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
  ];

  // 実行: 解凍を試行
  let result = Compression::gunzip_bytes(&invalid);

  // 検証: エラーが返される
  assert!(result.is_err());
}

// --- max_bytes_per_chunk_default 定数 ---

#[test]
fn max_bytes_per_chunk_default_is_64k() {
  // 検証: Pekko の MaxBytesPerChunkDefault (64 KiB) と一致
  assert_eq!(Compression::MAX_BYTES_PER_CHUNK_DEFAULT, 64 * 1024);
}

// --- nowrap オプション ---

#[test]
fn deflate_with_nowrap_round_trips() {
  // 準備: 入力バイト列
  let input: Vec<u8> = alloc::vec![42, 43, 44, 45];

  // 実行: nowrap=true で圧縮・解凍
  let compressed = Compression::deflate_bytes_with_options(&input, 6, true);
  let decompressed =
    Compression::inflate_bytes_with_options(&compressed, Compression::MAX_BYTES_PER_CHUNK_DEFAULT, true)
      .expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

#[test]
fn deflate_with_zlib_wrapper_round_trips() {
  // 準備: 入力バイト列
  let input: Vec<u8> = alloc::vec![1, 3, 5, 7, 9];

  // 実行: nowrap=false で圧縮・解凍
  let compressed = Compression::deflate_bytes_with_options(&input, 6, false);
  let decompressed =
    Compression::inflate_bytes_with_options(&compressed, Compression::MAX_BYTES_PER_CHUNK_DEFAULT, false)
      .expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}

// --- 明示的な max_bytes による解凍 ---

#[test]
fn gzip_decompress_with_max_bytes_accepts_within_limit() {
  // 準備: 小さい入力データ
  let input: Vec<u8> = alloc::vec![1, 2, 3];
  let compressed = Compression::gzip_bytes(&input);

  // 実行: 十分な制限で解凍
  let decompressed = Compression::gunzip_bytes_with_options(&compressed, 1024).expect("decompress");

  // 検証: 元の入力と一致
  assert_eq!(decompressed, input);
}
