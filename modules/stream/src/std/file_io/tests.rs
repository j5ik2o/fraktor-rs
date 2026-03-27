use std::io::Write;

use tempfile::NamedTempFile;

use crate::{
  core::{
    Completion, StreamError,
    buffer::StreamBufferConfig,
    lifecycle::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
    mat::{KeepBoth, KeepRight, Materialized, Materializer, RunnableGraph, StreamCompletion},
    stage::{Sink, Source},
  },
  std::FileIO,
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamShared::new(stream);
    let handle = StreamHandleImpl::new(StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

fn drive_to_completion<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..256 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
  }
  panic!("stream did not reach terminal state after 256 drive iterations");
}

// --- from_path tests ---

#[test]
fn from_path_reads_file_contents() {
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"hello").unwrap();
  tmp.flush().unwrap();

  let source = FileIO::from_path(tmp.path());
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 5);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![b'h', b'e', b'l', b'l', b'o'])));
}

#[test]
fn from_path_nonexistent_returns_failed_io_result() {
  let source = FileIO::from_path("/nonexistent/path/to/file.txt");
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, _completion) = materialized.materialized();
  assert!(!io_result.was_successful());
  assert_eq!(io_result.count(), 0);
}

#[test]
fn from_path_empty_file_returns_zero_count() {
  let tmp = NamedTempFile::new().unwrap();

  let source = FileIO::from_path(tmp.path());
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<u8>::new())));
}

// --- to_path tests ---

#[test]
fn to_path_writes_file_contents() {
  let tmp = NamedTempFile::new().unwrap();
  let path = tmp.path().to_path_buf();

  let source = Source::from_iterator(vec![b'w', b'o', b'r', b'l', b'd']);
  let sink = FileIO::to_path(&path);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 5);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  let written = std::fs::read(&path).unwrap();
  assert_eq!(written, b"world");
}

#[test]
fn to_path_empty_stream_writes_empty_file() {
  let tmp = NamedTempFile::new().unwrap();
  let path = tmp.path().to_path_buf();

  let source = Source::<u8, _>::empty();
  let sink = FileIO::to_path(&path);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 0);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  let written = std::fs::read(&path).unwrap();
  assert!(written.is_empty());
}

#[test]
fn to_path_invalid_directory_returns_failed_io_result() {
  let source = Source::from_iterator(vec![1_u8, 2, 3]);
  let sink = FileIO::to_path("/nonexistent/directory/file.txt");
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::IoError { .. })));
}

// --- from_path_with_options tests ---

#[test]
fn from_path_with_options_reads_partial_file_with_start_position() {
  // 準備: "abcdefghij" を含むファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"abcdefghij").unwrap();
  tmp.flush().unwrap();

  // 実行: start_position=3, chunk_size=4 で読み込み
  let source = FileIO::from_path_with_options(tmp.path(), 4, 3);
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: start_position=3 以降の全バイト "defghij" が読まれる
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 7);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![b'd', b'e', b'f', b'g', b'h', b'i', b'j'])));
}

#[test]
fn from_path_with_options_reads_from_start_when_position_is_zero() {
  // 準備: "hello" を含むファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"hello").unwrap();
  tmp.flush().unwrap();

  // 実行: position=0, ファイルより大きい chunk_size で読み込み
  let source = FileIO::from_path_with_options(tmp.path(), 8192, 0);
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: 全バイトが読まれる
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 5);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![b'h', b'e', b'l', b'l', b'o'])));
}

#[test]
fn from_path_with_options_returns_empty_when_position_past_end() {
  // 準備: "abc" を含むファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"abc").unwrap();
  tmp.flush().unwrap();

  // 実行: ファイル末尾を超える位置から読み込み
  let source = FileIO::from_path_with_options(tmp.path(), 100, 999);
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: 0 バイトが読まれる
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<u8>::new())));
}

#[test]
fn from_path_with_options_rejects_zero_chunk_size() {
  // 準備: 入力ファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"abc").unwrap();
  tmp.flush().unwrap();

  // 実行: chunk_size=0 を指定
  let source = FileIO::from_path_with_options(tmp.path(), 0, 0);
  let graph = source.to_mat(Sink::<u8, StreamCompletion<alloc::vec::Vec<u8>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: InvalidInput として失敗する
  let (io_result, completion) = materialized.materialized();
  assert!(!io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert!(matches!(io_result.error(), Some(StreamError::IoError { .. })));
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<u8>::new())));
}

// --- to_path_with_options tests ---

#[test]
fn to_path_with_options_appends_to_existing_file() {
  // 準備: "hello" を含むファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"hello").unwrap();
  tmp.flush().unwrap();
  let path = tmp.path().to_path_buf();

  // 実行: append オプションで " world" を書き込み
  let mut options = std::fs::OpenOptions::new();
  options.write(true).append(true);
  let source = Source::from_iterator(vec![b' ', b'w', b'o', b'r', b'l', b'd']);
  let sink = FileIO::to_path_with_options(&path, options);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: ファイルが "hello world" を含む
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 6);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }
  let written = std::fs::read(&path).unwrap();
  assert_eq!(written, b"hello world");
}

#[test]
fn to_path_with_options_creates_new_file() {
  // 準備: 新規ファイルのパス
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("new_file.bin");

  // 実行: create + write オプションでバイトを書き込み
  let mut options = std::fs::OpenOptions::new();
  options.write(true).create(true).truncate(true);
  let source = Source::from_iterator(vec![1_u8, 2, 3]);
  let sink = FileIO::to_path_with_options(&path, options);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: ファイルが作成され内容が一致
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 3);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }
  let written = std::fs::read(&path).unwrap();
  assert_eq!(written, vec![1, 2, 3]);
}

// --- to_path_with_position tests ---

#[test]
fn to_path_with_position_writes_at_offset() {
  // 準備: "AAAAAAAAAA" (10 バイト) を含むファイル
  let mut tmp = NamedTempFile::new().unwrap();
  tmp.write_all(b"AAAAAAAAAA").unwrap();
  tmp.flush().unwrap();
  let path = tmp.path().to_path_buf();

  // 実行: start_position=3 に "BB" を書き込み
  let mut options = std::fs::OpenOptions::new();
  options.write(true);
  let source = Source::from_iterator(vec![b'B', b'B']);
  let sink = FileIO::to_path_with_position(&path, options, 3);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).unwrap();
  drive_to_completion(&materialized);

  // 検証: ファイルが "AAABBAAAAA" を含む
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 2);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }
  let written = std::fs::read(&path).unwrap();
  assert_eq!(written, b"AAABBAAAAA");
}

#[test]
fn to_path_with_position_invalid_path_returns_failed_io_result() {
  // 準備: 存在しないディレクトリ
  let mut options = std::fs::OpenOptions::new();
  options.write(true);

  // 実行: 存在しないパスに position 指定で書き込み
  let source = Source::from_iterator(vec![1_u8, 2]);
  let sink = FileIO::to_path_with_position("/nonexistent/dir/file.bin", options, 0);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let result = graph.run(&mut materializer);

  // 検証: 起動時に IO エラーを返す
  assert!(matches!(result, Err(StreamError::IoError { .. })));
}
