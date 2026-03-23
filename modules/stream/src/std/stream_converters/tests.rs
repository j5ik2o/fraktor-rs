use std::io;

use crate::{
  core::{
    Completion, KeepBoth, KeepRight, StreamBufferConfig, StreamCompletion, StreamError,
    lifecycle::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
    mat::{Materialized, Materializer, RunnableGraph},
    stage::{Sink, Source},
  },
  std::StreamConverters,
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

const MAX_DRIVE_ITERATIONS: usize = 256;

fn drive_to_completion<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..MAX_DRIVE_ITERATIONS {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
  }
  panic!("ストリームが {MAX_DRIVE_ITERATIONS} 回の drive で終了状態に達しなかった");
}

// --- from_reader テスト ---

#[test]
fn from_reader_reads_all_bytes_as_chunks() {
  // Given: a reader producing "hello world" (11 bytes) with chunk_size 4
  let source = StreamConverters::from_reader(|| Box::new(io::Cursor::new(b"hello world".to_vec())), 4);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: all chunks are emitted and IOResult reports total bytes
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 11);

  // Flatten chunks to verify all data was read
  match completion.poll() {
    | Completion::Ready(Ok(chunks)) => {
      let flat: alloc::vec::Vec<u8> = chunks.into_iter().flatten().collect();
      assert_eq!(flat, b"hello world");
    },
    | other => panic!("expected Ready(Ok(chunks)), got {other:?}"),
  }
}

#[test]
fn from_reader_empty_reader_returns_zero_count() {
  // Given: an empty reader
  let source = StreamConverters::from_reader(|| Box::new(io::Cursor::new(alloc::vec::Vec::<u8>::new())), 8192);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: IOResult reports 0 bytes and no chunks emitted
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<alloc::vec::Vec<u8>>::new())));
}

#[test]
fn from_reader_single_chunk_when_data_fits() {
  // Given: a reader with exactly 5 bytes and chunk_size 8192
  let source = StreamConverters::from_reader(|| Box::new(io::Cursor::new(b"abcde".to_vec())), 8192);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: exactly one chunk of 5 bytes is emitted
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 5);
  match completion.poll() {
    | Completion::Ready(Ok(chunks)) => {
      assert_eq!(chunks.len(), 1);
      assert_eq!(chunks[0], b"abcde");
    },
    | other => panic!("expected Ready(Ok(chunks)), got {other:?}"),
  }
}

#[test]
fn from_reader_exact_chunk_boundary() {
  // Given: a reader with exactly 8 bytes and chunk_size 4
  let source = StreamConverters::from_reader(|| Box::new(io::Cursor::new(b"abcdefgh".to_vec())), 4);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: exactly two chunks of 4 bytes each are emitted
  let (io_result, completion) = materialized.materialized();
  assert!(io_result.was_successful());
  assert_eq!(io_result.count(), 8);
  match completion.poll() {
    | Completion::Ready(Ok(chunks)) => {
      assert_eq!(chunks.len(), 2);
      assert_eq!(chunks[0], b"abcd");
      assert_eq!(chunks[1], b"efgh");
    },
    | other => panic!("expected Ready(Ok(chunks)), got {other:?}"),
  }
}

#[test]
fn from_reader_io_error_returns_failed_io_result() {
  // Given: a reader that fails immediately
  struct FailingReader;
  impl io::Read for FailingReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
      Err(io::Error::new(io::ErrorKind::BrokenPipe, "test error"))
    }
  }

  let source = StreamConverters::from_reader(|| Box::new(FailingReader), 4096);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: IOResult reports failure with 0 bytes read
  let (io_result, _completion) = materialized.materialized();
  assert!(!io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert!(matches!(io_result.error(), Some(StreamError::IoError { .. })));
}

#[test]
fn from_reader_rejects_zero_chunk_size() {
  // 準備: 読み込み元は有効だが chunk_size=0
  let source = StreamConverters::from_reader(|| Box::new(io::Cursor::new(b"hello".to_vec())), 0);
  let graph = source
    .to_mat(Sink::<alloc::vec::Vec<u8>, StreamCompletion<alloc::vec::Vec<alloc::vec::Vec<u8>>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // 検証: InvalidInput として失敗する
  let (io_result, completion) = materialized.materialized();
  assert!(!io_result.was_successful());
  assert_eq!(io_result.count(), 0);
  assert!(matches!(io_result.error(), Some(StreamError::IoError { .. })));
  assert_eq!(completion.poll(), Completion::Ready(Ok(alloc::vec::Vec::<alloc::vec::Vec<u8>>::new())));
}

// --- to_writer テスト ---

#[test]
fn to_writer_writes_all_bytes() {
  // Given: a writer backed by a Vec<u8>
  use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

  let buffer = ArcShared::new(SpinSyncMutex::new(alloc::vec::Vec::<u8>::new()));
  let sink = StreamConverters::to_writer(
    {
      let buffer = buffer.clone();
      move || Box::new(SharedWriter(buffer)) as Box<dyn std::io::Write + Send>
    },
    false,
  );

  let source = Source::from_iterator(vec![b'h', b'e', b'l', b'l', b'o']);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: IOResult reports 5 bytes written
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 5);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  // And: the buffer contains the written bytes
  assert_eq!(*buffer.lock(), b"hello");
}

#[test]
fn to_writer_empty_stream_writes_nothing() {
  // Given: a writer backed by a Vec<u8>
  use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

  let buffer = ArcShared::new(SpinSyncMutex::new(alloc::vec::Vec::<u8>::new()));
  let sink = StreamConverters::to_writer(
    {
      let buffer = buffer.clone();
      move || Box::new(SharedWriter(buffer)) as Box<dyn std::io::Write + Send>
    },
    false,
  );

  let source = Source::<u8, _>::empty();
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: IOResult reports 0 bytes written
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 0);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  assert!(buffer.lock().is_empty());
}

#[test]
fn to_writer_with_auto_flush_flushes_after_each_element() {
  // Given: a flush-counting writer
  use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

  let flush_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let sink = StreamConverters::to_writer(
    {
      let flush_count = flush_count.clone();
      move || Box::new(FlushCountingWriter { flush_count }) as Box<dyn std::io::Write + Send>
    },
    true,
  );

  let source = Source::from_iterator(vec![1_u8, 2, 3]);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: flush was called at least 3 times (once per element)
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(io_result.was_successful());
      assert_eq!(io_result.count(), 3);
    },
    | other => panic!("expected Ready(Ok(IOResult)), got {other:?}"),
  }

  assert!(*flush_count.lock() >= 3);
}

#[test]
fn to_writer_io_error_returns_failed_io_result() {
  // Given: a writer that fails on write
  let sink = StreamConverters::to_writer(|| Box::new(FailingWriter) as Box<dyn std::io::Write + Send>, false);

  let source = Source::from_iterator(vec![1_u8, 2, 3]);
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_to_completion(&materialized);

  // Then: IOResult reports failure
  let completion = materialized.materialized();
  match completion.poll() {
    | Completion::Ready(Ok(io_result)) => {
      assert!(!io_result.was_successful());
      assert!(matches!(io_result.error(), Some(StreamError::IoError { .. })));
    },
    | other => panic!("expected Ready(Ok(IOResult::failed)), got {other:?}"),
  }
}

// --- テストヘルパー型 ---

/// Writer backed by a shared Vec<u8> for verifying written bytes.
struct SharedWriter(
  fraktor_utils_rs::core::sync::ArcShared<
    fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<alloc::vec::Vec<u8>>,
  >,
);

impl std::io::Write for SharedWriter {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// Writer that counts flush invocations.
struct FlushCountingWriter {
  flush_count:
    fraktor_utils_rs::core::sync::ArcShared<fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<u32>>,
}

impl std::io::Write for FlushCountingWriter {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    *self.flush_count.lock() += 1;
    Ok(())
  }
}

/// Writer that always fails.
struct FailingWriter;

impl std::io::Write for FailingWriter {
  fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
    Err(io::Error::new(io::ErrorKind::BrokenPipe, "test write error"))
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}
